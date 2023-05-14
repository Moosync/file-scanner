use lazy_static::lazy_static;
use lofty::{read_from_path, Accessor, AudioFile, Picture, Probe, TaggedFileExt};
use regex::Regex;
use std::{
  fs::{self, File},
  io::{self, BufRead},
  num::NonZeroU32,
  path::{Path, PathBuf},
  sync::{
    atomic::{AtomicUsize, Ordering},
    mpsc::Sender,
  },
};
use substring::Substring;
use threadpool::ThreadPool;

use image::ColorType;

use crate::{
  database::files_not_in_db,
  error::ScanError,
  structs::{FileList, Playlist, Song},
};
use fast_image_resize as fr;

fn get_files_recursively(dir: PathBuf) -> Result<FileList, ScanError> {
  let mut file_list: Vec<(PathBuf, u64)> = vec![];
  let mut playlist_list: Vec<PathBuf> = vec![];

  let dir_entries = fs::read_dir(dir)?;

  lazy_static! {
    static ref SONG_RE: Regex = Regex::new("flac|mp3|ogg|m4a|webm|wav|wv|aac|opus").unwrap();
    static ref PLAYLIST_RE: Regex = Regex::new("m3u|m3u8").unwrap();
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
    playlist_list,
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

pub fn scan_playlist(path: &PathBuf) -> Result<(Playlist, Vec<Song>), ScanError> {
  let file = File::open(path)?;
  let lines = io::BufReader::new(file).lines();

  let mut songs: Vec<Song> = vec![];

  let mut song_type: Option<String> = None;
  let mut duration: Option<f64> = None;
  let mut title: Option<String> = None;
  let mut artists: Option<String> = None;
  let mut playlist_title: String = "".to_string();
  for line_res in lines {
    if let Ok(line) = line_res {
      if line.starts_with("#EXTINF:") {
        let metadata = line.substring(8, line.len());
        let split_index = metadata.find(",").unwrap_or_default();

        duration = Some(metadata.substring(0, split_index).parse::<f64>()?);

        let non_duration = metadata.substring(split_index, metadata.len());
        let (artists_str, title_str) = non_duration.split_at(non_duration.find("-").unwrap());
        artists = Some(artists_str.to_string());
        title = Some(title_str.to_string());

        continue;
      }

      if line.starts_with("#MOOSINF:") {
        song_type = Some(line.substring(9, line.len()).to_string());
        continue;
      }

      if line.starts_with("#PLAYLIST:") {
        playlist_title = line.substring(10, line.len()).to_string();
        continue;
      }

      if !line.starts_with("#") {
        let mut song = Song::default();
        song.path = Some(line);
        song.artists = artists;
        song.duration = duration;
        song.title = title;
        song.song_type = song_type;

        songs.push(song);

        artists = None;
        duration = None;
        title = None;
        song_type = None;
      }
    }
  }

  Ok((
    Playlist {
      id: get_id(),
      title: playlist_title,
    },
    songs,
  ))
}

fn scan_file(
  path: &PathBuf,
  thumbnail_dir: &PathBuf,
  playlist_id: Option<u32>,
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
  song.duration = Some(properties.duration().as_secs() as f64);
  song.path = Some(path.to_string_lossy().to_string());
  song.size = Some(size as u32);
  song.playlist_id = playlist_id;

  if tags.is_some() {
    let metadata = tags.unwrap();

    let picture = metadata.pictures().get(0);
    if picture.is_some() {
      let (high_path, low_path) = store_picture(thumbnail_dir, picture.unwrap())?;
      song.high_path = Some(high_path.to_str().unwrap_or_default().to_string());
      song.low_path = Some(low_path.to_str().unwrap_or_default().to_string());
    } else {
      let mut base_path = path.clone();
      base_path.pop();
      let files_res = base_path.read_dir();
      if let Ok(mut files) = files_res {
        song.high_path = files.find_map(|e| {
          if let Ok(dir_entry) = e {
            let file_name = dir_entry
              .path()
              .file_stem()
              .unwrap_or_default()
              .to_string_lossy()
              .to_lowercase();

            if file_name.starts_with("cover") {
              return Some(dir_entry.path().to_string_lossy().to_string());
            }
          }
          None
        });
      }
    }

    song.title = metadata.title().map(|s| s.to_string());
    song.album = metadata.album().map(|s| s.to_string());
    song.artists = metadata.artist().map(|s| s.to_string());
    song.year = metadata.year().map(|s| s.to_string());
    song.genre = metadata.genre().map(|s| s.to_string());
    song.lyrics = metadata
      .get_string(&lofty::ItemKey::Lyrics)
      .map(str::to_string);
    song.track_no = metadata
      .get_string(&lofty::ItemKey::TrackNumber)
      .map(str::to_string);
  }

  Ok(song)
}

fn check_directory(dir: PathBuf) -> Result<(), ScanError> {
  if !dir.is_dir() {
    fs::create_dir_all(dir)?
  }

  Ok(())
}

fn scan_in_pool(
  pool: &ThreadPool,
  tx: Sender<Result<Song, ScanError>>,
  size: u64,
  path: PathBuf,
  thumbnail_dir: PathBuf,
  playlist_id: Option<u32>,
) -> &ThreadPool {
  pool.execute(move || {
    let mut metadata = scan_file(&path, &thumbnail_dir, playlist_id, size, false);
    if metadata.is_err() {
      println!("Guessing filetype");
      metadata = scan_file(&path, &thumbnail_dir, playlist_id, size, true);
    }

    tx.send(metadata)
      .expect("channel will be there waiting for the pool");
  });

  pool
}

pub fn start_scan(
  dir: PathBuf,
  thumbnail_dir: PathBuf,
  database: PathBuf,
  tx_song: Sender<Result<Song, ScanError>>,
  tx_playlist: Sender<Result<Playlist, ScanError>>,
  mut pool: &ThreadPool,
) -> Result<&ThreadPool, ScanError> {
  check_directory(dir.clone())?;
  check_directory(thumbnail_dir.clone())?;

  let file_list = get_files_recursively(dir)?;
  let song_list = files_not_in_db(database, file_list.file_list).unwrap();

  for playlist in file_list.playlist_list {
    let (playlist_dets, songs) = scan_playlist(&playlist)?;
    let tx_p = tx_playlist.clone();
    tx_p
      .send(Ok(playlist_dets.clone()))
      .expect("channel will be there waiting for the pool");

    for s in songs {
      let tx = tx_song.clone();
      if s.song_type.is_none() || s.song_type.is_some() && s.song_type.unwrap() == "LOCAL" {
        let thumbnail_dir = thumbnail_dir.clone();
        let mut path = PathBuf::new();
        path.push(playlist.clone());
        path.pop();
        path.push(s.path.unwrap());

        pool = scan_in_pool(&pool, tx, 0, path, thumbnail_dir, Some(playlist_dets.id));
      }
    }
  }

  for (file_path, size) in song_list {
    let thumbnail_dir = thumbnail_dir.clone();
    let tx = tx_song.clone();
    pool = scan_in_pool(&pool, tx, size, file_path, thumbnail_dir, None);
  }

  drop(tx_song);
  Ok(pool)
}

fn get_id() -> u32 {
  static COUNTER: AtomicUsize = AtomicUsize::new(1);
  COUNTER.fetch_add(1, Ordering::Relaxed) as u32
}
