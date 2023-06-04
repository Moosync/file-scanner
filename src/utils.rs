use lazy_static::lazy_static;
use lofty::{read_from_path, Accessor, AudioFile, Picture, Probe, TaggedFileExt};
use regex::Regex;
use std::{
  fs::{self},
  num::NonZeroU32,
  path::{Path, PathBuf},
};
use uuid::Uuid;

use image::ColorType;

use crate::{
  error::ScanError,
  structs::{Album, Artists, FileList, Song},
};
use fast_image_resize as fr;

pub fn check_directory(dir: PathBuf) -> Result<(), ScanError> {
  println!("{:?} {:?}", dir, dir.is_dir());
  if !dir.is_dir() {
    fs::create_dir_all(dir)?
  }

  Ok(())
}

pub fn get_files_recursively(dir: PathBuf) -> Result<FileList, ScanError> {
  let mut file_list: Vec<(PathBuf, u64)> = vec![];
  let mut playlist_list: Vec<PathBuf> = vec![];

  lazy_static! {
    static ref SONG_RE: Regex = Regex::new("flac|mp3|ogg|m4a|webm|wav|wv|aac|opus").unwrap();
    static ref PLAYLIST_RE: Regex = Regex::new("m3u|m3u8").unwrap();
  }

  if !dir.exists() {
    return Ok(FileList {
      file_list,
      playlist_list,
    });
  }

  if dir.is_file() {
    if let Ok(metadata) = fs::metadata(&dir) {
      let extension = dir
        .extension()
        .unwrap_or_default()
        .to_str()
        .unwrap_or_default();
      if !extension.is_empty() {
        if SONG_RE.is_match(extension) {
          file_list.push((dir.clone(), metadata.len()));
        }

        if PLAYLIST_RE.is_match(extension) {
          playlist_list.push(dir);
        }
      }
      return Ok(FileList {
        file_list,
        playlist_list,
      });
    }
  }

  let dir_entries = fs::read_dir(dir)?;

  for entry in dir_entries {
    if let Ok(entry) = entry {
      let path = entry.path();

      if let Ok(metadata) = fs::metadata(&path) {
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

  Ok((
    dunce::canonicalize(high_path)?,
    dunce::canonicalize(low_path)?,
  ))
}

fn scan_lrc(mut path: PathBuf) -> Option<String> {
  path.set_extension("lrc");
  if path.exists() {
    lazy_static! {
      static ref LRC_REGEX: Regex = Regex::new(r"\[\d{2}:\d{2}.\d{2}\]").unwrap();
    }

    let data = fs::read(path);
    if data.is_err() {
      return None;
    }

    let mut parsed_lyrics = "".to_string();
    let parsed = String::from_utf8_lossy(&data.unwrap()).to_string();
    for line in parsed.split("\n") {
      if LRC_REGEX.is_match(line) {
        parsed_lyrics.push_str(&LRC_REGEX.replace_all(line, ""));
        parsed_lyrics.push('\n');
      }
    }

    return Some(parsed_lyrics);
  }

  None
}

pub fn scan_file(
  path: &PathBuf,
  thumbnail_dir: &PathBuf,
  playlist_id: &Option<String>,
  size: u64,
  guess: bool,
  artist_split: &str,
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
  song.path = Some(dunce::canonicalize(path)?.to_string_lossy().to_string());
  song.size = Some(size as u32);
  song.playlist_id = playlist_id.clone();
  song._id = Uuid::new_v4().to_string();

  if tags.is_some() {
    let metadata = tags.unwrap();

    let picture = metadata.pictures().get(0);
    if picture.is_some() {
      if let Ok((high_path, low_path)) = store_picture(thumbnail_dir, picture.unwrap()) {
        song.high_path = Some(high_path.to_string_lossy().to_string());
        song.low_path = Some(low_path.to_string_lossy().to_string());
      }
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

    let mut lyrics = metadata
      .get_string(&lofty::ItemKey::Lyrics)
      .map(str::to_string);

    if lyrics.is_none() {
      lyrics = scan_lrc(path.clone());
    }

    song.title = metadata.title().map(|s| s.to_string());
    // song.album = metadata.album().map(|s| s.to_string());
    let artists: Option<Vec<Artists>> = metadata.artist().map(|s| {
      s.split(artist_split)
        .map(|s| Artists {
          artist_id: Uuid::new_v4().to_string(),
          artist_name: s.trim().to_string(),
        })
        .collect()
    });

    let album = metadata.album();
    if album.is_some() {
      song.album = Some(Album {
        album_id: Uuid::new_v4().to_string(),
        album_name: album.unwrap().to_string(),
        album_cover_path_high: song.high_path.clone(),
        album_cover_path_low: song.low_path.clone(),
      })
    }

    if artists.is_some() {
      song.artists = artists.unwrap();
    }

    song.year = metadata.year().map(|s| s.to_string());
    song.genre = metadata.genre().map(|s| vec![s.to_string()]);
    song.lyrics = lyrics;
    song.song_type = "LOCAL".to_string();
  }

  Ok(song)
}
