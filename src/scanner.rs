use lazy_static::lazy_static;
use lofty::{read_from_path, Accessor, AudioFile, Picture, Probe, TaggedFileExt};
use regex::Regex;
use std::{
  fs,
  num::NonZeroU32,
  os::unix::prelude::MetadataExt,
  path::{Path, PathBuf},
  sync::mpsc::Sender,
  time::Instant,
};
use threadpool::ThreadPool;

use image::ColorType;

use crate::{database::files_not_in_db, error::ScanError};
use fast_image_resize as fr;

#[derive(Default, Debug)]
#[napi(object)]
pub struct Song {
  pub bitrate: Option<u32>,
  pub sample_rate: Option<u32>,
  pub duration: Option<u32>,
  pub path: String,
  pub size: u32,
  pub title: String,
  pub album: String,
  pub artists: String,
  pub year: String,
  pub genre: String,
  pub lyrics: String,
  pub track_no: String,
}

fn get_files_recursively(dir: PathBuf) -> Result<Vec<(PathBuf, u64)>, ScanError> {
  let mut ret: Vec<(PathBuf, u64)> = vec![];

  let dir_entries = fs::read_dir(dir)?;

  lazy_static! {
    static ref RE: Regex = Regex::new("flac|mp3|ogg|m4a|webm|wav|wv|aac|opus").unwrap();
  }

  for entry in dir_entries {
    let entry = entry?;
    let path = entry.path();

    let metadata = fs::metadata(&path)?;

    if metadata.is_dir() {
      let res = get_files_recursively(path)?;
      ret.extend_from_slice(&res);
      continue;
    }

    if metadata.is_file() {
      let extension = path
        .extension()
        .unwrap_or_default()
        .to_str()
        .unwrap_or_default();

      if !extension.is_empty() && RE.is_match(extension) {
        ret.push((path, metadata.len()));
      }
    }
  }
  Ok(ret)
}

fn store_picture(thumbnail_dir: &PathBuf, picture: &Picture) -> Result<(), ScanError> {
  let data = picture.data();
  let hash = blake3::hash(&data).to_hex();
  let hash_str = hash.as_str();

  let low_path = thumbnail_dir.join(format!("{}-low.png", hash_str));
  let high_path = thumbnail_dir.join(format!("{}.png", hash_str));

  if Path::new(high_path.to_str().unwrap()).exists() {
    return Ok(());
  }

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
    low_path,
    dst_image.buffer(),
    dst_width.get(),
    dst_height.get(),
    ColorType::Rgba8,
  )?;

  std::fs::write(high_path, data).expect("Couldnt write to file");
  Ok(())
}

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

    match metadata.pictures().get(0) {
      Some(p) => store_picture(thumbnail_dir, p)?,
      None => {}
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
  tx: Sender<Result<u16, ScanError>>,
) -> Result<ThreadPool, ScanError> {
  check_directory(dir.clone())?;
  check_directory(thumbnail_dir.clone())?;

  let start = Instant::now();
  let mut file_list = get_files_recursively(dir)?;
  file_list = files_not_in_db(database, file_list.clone()).unwrap();
  let duration = start.elapsed();
  println!("Time elapsed in expensive_function() is: {:?}", duration);

  let pool = ThreadPool::new(12);
  let thumbnail_dir_2 = thumbnail_dir.clone();
  for (file_path, size) in file_list {
    let thumbnail_dir_1 = thumbnail_dir_2.clone();
    let tx = tx.clone();
    pool.execute(move || {
      let start = Instant::now();
      let mut metadata = scan_file(file_path.clone(), &thumbnail_dir_1, size, false);
      if metadata.is_err() {
        println!("Guessing filetype");
        metadata = scan_file(file_path, &thumbnail_dir_1, size, true);
      }

      println!("{:?}", metadata);
      tx.send(Ok(1))
        .expect("channel will be there waiting for the pool");
      // tx.send(metadata)
      //   .expect("channel will be there waiting for the pool");

      let duration = start.elapsed();
      println!("Time elapsed in scan is: {:?}", duration);
    });
  }

  drop(tx);
  Ok(pool)
}
