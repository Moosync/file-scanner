use lazy_static::lazy_static;
use lofty::{read_from_path, Accessor, AudioFile, Picture, Probe, TaggedFileExt};
use regex::Regex;
use std::{
  fs,
  num::NonZeroU32,
  path::{Path, PathBuf},
  sync::mpsc::Sender,
  time::Instant,
};
use threadpool::ThreadPool;

use image::ColorType;

use crate::{
  database::files_not_in_db,
  error::ScanError,
  structs::{FileList, Song},
};
use fast_image_resize as fr;

fn get_files_recursively(dir: PathBuf) -> Result<FileList, ScanError> {
  let mut file_list: Vec<(PathBuf, u64)> = vec![];
  let mut playlist_list: Vec<PathBuf> = vec![];

  let dir_entries = fs::read_dir(dir)?;

  lazy_static! {
    static ref SONG_RE: Regex = Regex::new("flac|mp3|ogg|m4a|webm|wav|wv|aac|opus").unwrap();
    static ref PLAYLIST_RE: Regex = Regex::new("m3u").unwrap();
  }

  for entry in dir_entries {
    let entry = entry?;
    let path = entry.path();

    let metadata = fs::metadata(&path)?;

    if metadata.is_dir() {
      let res = get_files_recursively(path)?;
      file_list.extend_from_slice(&res.file_list);
      playlist_list.extend_from_slice(&res.playlist_list);
      continue;
    }

    if metadata.is_file() {
      let extension = path
        .extension()
        .unwrap_or_default()
        .to_str()
        .unwrap_or_default();

      if !extension.is_empty() {
        if SONG_RE.is_match(extension) {
          file_list.push((path.clone(), metadata.len()));
        }

        if PLAYLIST_RE.is_match(extension) {
          playlist_list.push(path);
        }
      }
    }
  }

  Ok(FileList {
    file_list,
    playlist_list: vec![],
  })
}

fn store_picture(
  thumbnail_dir: &PathBuf,
  picture: &Picture,
) -> Result<(PathBuf, PathBuf), ScanError> {
  let data = picture.data();
  let hash = blake3::hash(&data).to_hex();
  let hash_str = hash.as_str();

  let low_path = thumbnail_dir.join(format!("{}-low.png", hash_str));
  let high_path = thumbnail_dir.join(format!("{}.png", hash_str));

  if !Path::new(high_path.to_str().unwrap()).exists() {
    std::fs::write(high_path.clone(), data).expect("Couldnt write to file");
  }

  if !Path::new(low_path.to_str().unwrap()).exists() {
    let img = image::load_from_memory(&data)?;

    let width = NonZeroU32::new(img.width()).unwrap();
    let height = NonZeroU32::new(img.height()).unwrap();
    let src_image = fr::Image::from_vec_u8(
      width,
      height,
      img.to_rgba8().into_raw(),
      fr::PixelType::U8x4,
    )
    .unwrap();

    // Create container for data of destination image
    let dst_width = NonZeroU32::new(80).unwrap();
    let dst_height = NonZeroU32::new(80).unwrap();
    let mut dst_image = fr::Image::new(dst_width, dst_height, src_image.pixel_type());

    // Get mutable view of destination image data
    let mut dst_view = dst_image.view_mut();

    // Create Resizer instance and resize source image
    // into buffer of destination image
    let mut resizer = fr::Resizer::new(fr::ResizeAlg::Nearest);
    resizer.resize(&src_image.view(), &mut dst_view)?;

    image::save_buffer(
      low_path.clone(),
      dst_image.buffer(),
      dst_width.get(),
      dst_height.get(),
      ColorType::Rgba8,
    )?;
  }

  Ok((high_path, low_path))
}

pub fn scan_playlist() {}

fn scan_file(
  path: PathBuf,
  thumbnail_dir: &PathBuf,
  size: u64,
  guess: bool,
) -> Result<Song, ScanError> {
  let file: lofty::TaggedFile;

  if guess {
    file = read_from_path(path.clone())?;
  } else {
    file = Probe::open(path.clone())?.guess_file_type()?.read()?;
  }

  let properties = file.properties();
  let mut tags = file.primary_tag();
  if tags.is_none() {
    tags = file.first_tag();
  }
  let mut song = Song::default();
  song.bitrate = Some(properties.audio_bitrate().unwrap_or_default() * 1000);
  song.sample_rate = properties.sample_rate();
  song.duration = Some(properties.duration().as_secs() as u32);
  song.path = path.to_string_lossy().to_string();
  song.size = size as u32;

  if tags.is_some() {
    let metadata = tags.unwrap();

    let picture = metadata.pictures().get(0);
    if picture.is_some() {
      let (high_path, low_path) = store_picture(thumbnail_dir, picture.unwrap())?;
      song.high_path = Some(high_path.to_str().unwrap_or_default().to_string());
      song.low_path = Some(low_path.to_str().unwrap_or_default().to_string());
    };

    song.title = metadata.title().unwrap_or_default().to_string();
    song.album = metadata.album().unwrap_or_default().to_string();
    song.artists = metadata.artist().unwrap_or_default().to_string();
    song.year = metadata.year().unwrap_or_default().to_string();
    song.genre = metadata.genre().unwrap_or_default().to_string();
    song.lyrics = metadata
      .get_string(&lofty::ItemKey::Lyrics)
      .unwrap_or_default()
      .to_string();
    song.track_no = metadata
      .get_string(&lofty::ItemKey::TrackNumber)
      .unwrap_or_default()
      .to_string();
  }

  Ok(song)
}

fn check_directory(dir: PathBuf) -> Result<(), ScanError> {
  if !dir.is_dir() {
    fs::create_dir_all(dir)?
  }

  Ok(())
}

pub fn start_scan(
  dir: PathBuf,
  thumbnail_dir: PathBuf,
  database: PathBuf,
  tx: Sender<Result<Song, ScanError>>,
) -> Result<ThreadPool, ScanError> {
  check_directory(dir.clone())?;
  check_directory(thumbnail_dir.clone())?;

  let start = Instant::now();
  let file_list = get_files_recursively(dir)?;
  let song_list = files_not_in_db(database, file_list.file_list).unwrap();
  let duration = start.elapsed();
  println!("Time elapsed in expensive_function() is: {:?}", duration);

  let pool = ThreadPool::new(num_cpus::get());
  let thumbnail_dir_2 = thumbnail_dir.clone();
  for (file_path, size) in song_list {
    let thumbnail_dir_1 = thumbnail_dir_2.clone();
    let tx = tx.clone();
    pool.execute(move || {
      let start = Instant::now();
      let mut metadata = scan_file(file_path.clone(), &thumbnail_dir_1, size, false);
      if metadata.is_err() {
        println!("Guessing filetype");
        metadata = scan_file(file_path, &thumbnail_dir_1, size, true);
      }

      tx.send(metadata)
        .expect("channel will be there waiting for the pool");

      let duration = start.elapsed();
      println!("Time elapsed in scan is: {:?}", duration);
    });
  }

  drop(tx);
  Ok(pool)
}
