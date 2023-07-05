use std::{
  fs::{self, File},
  io::{self, BufRead},
  path::PathBuf,
  str::FromStr,
  sync::mpsc::Sender,
};

use substring::Substring;

use uuid::Uuid;

use crate::{
  error::ScanError,
  song_scanner::SongScanner,
  structs::{Artists, Playlist, Song},
  utils::{check_directory, get_files_recursively},
};

pub struct PlaylistScanner<'a> {
  dir: PathBuf,
  song_scanner: SongScanner<'a>,
  thumbnail_dir: PathBuf,
}

impl<'a> PlaylistScanner<'a> {
  pub fn new(dir: PathBuf, thumbnail_dir: PathBuf, song_scanner: SongScanner<'a>) -> Self {
    Self {
      dir,
      thumbnail_dir,
      song_scanner,
    }
  }

  fn check_dirs(&self) -> Result<(), ScanError> {
    check_directory(self.thumbnail_dir.clone())
  }

  fn parse_artists(&self, artists: Option<String>) -> Vec<Artists> {
    let mut ret: Vec<Artists> = vec![];
    if artists.is_some() {
      for artist in artists.unwrap().split(";") {
        ret.push(Artists {
          artist_id: Uuid::new_v4().to_string(),
          artist_name: artist.to_string(),
        })
      }
    }
    ret
  }

  fn scan_playlist(&self, path: &PathBuf) -> Result<(Playlist, Vec<Song>), ScanError> {
    let file = File::open(path)?;
    let lines = io::BufReader::new(file).lines();

    let mut songs: Vec<Song> = vec![];

    let mut song_type: Option<String> = None;
    let mut duration: Option<f64> = None;
    let mut title: Option<String> = None;
    let mut artists: Option<String> = None;
    let mut playlist_title: String = "".to_string();

    let playlist_id = Uuid::new_v4().to_string();
    for line_res in lines {
      if let Ok(mut line) = line_res {
        if line.starts_with("#EXTINF:") {
          let metadata = line.substring(8, line.len());
          let split_index = metadata.find(",").unwrap_or_default();

          duration = Some(metadata.substring(0, split_index).parse::<f64>()?);

          let non_duration = metadata.substring(split_index + 1, metadata.len());
          let (artists_str, title_str) = non_duration.split_at(non_duration.find("-").unwrap() - 1);
          artists = Some(artists_str.trim().to_string());
          title = Some(title_str.replacen("-", "", 1).trim().to_string());

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
          if line.starts_with("file://") {
            line = line[8..].to_string();
          }

          let mut song = Song::default();

          let s_type = song_type.clone();

          song.song_type = s_type.unwrap_or("LOCAL".to_string());
          song._id = Uuid::new_v4().to_string();

          if song.song_type == "LOCAL" {
            let song_path = PathBuf::from_str(line.as_str());
            if let Ok(mut path_parsed) = song_path {
              if path_parsed.is_relative() {
                path_parsed = path.parent().unwrap().join(path_parsed).canonicalize()?;
              }

              if !path_parsed.exists() {
                artists = None;
                duration = None;
                title = None;
                song_type = None;
                continue;
              }

              let metadata = fs::metadata(&path_parsed)?;
              song.size = Some(metadata.len() as u32);
              song.path = Some(path_parsed.to_string_lossy().to_string());
            }

            if song.path.is_none() {
              song.path = Some(line);
            }

            song.playback_url = None;
          } else {
            song._id = format!("{}:{}", song.song_type, line);
            song.playback_url = Some(line);
          }

          song.artists = self.parse_artists(artists);
          song.duration = duration;
          song.title = title;
          song.playlist_id = Some(playlist_id.clone());
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
        id: playlist_id,
        title: playlist_title,
        path: path.to_string_lossy().to_string(),
      },
      songs,
    ))
  }

  fn scan_song_in_pool(&self, tx_song: Sender<Result<Song, ScanError>>, s: Song) {
    if s.song_type == "LOCAL" {
      self.song_scanner.scan_in_pool(
        tx_song,
        s.size.unwrap() as u64,
        PathBuf::from_str(s.path.unwrap().as_str()).unwrap(),
        s.playlist_id,
      )
    } else {
      tx_song
        .send(Ok(s))
        .expect("channel will be there waiting for the pool");
    }
  }

  pub fn start(
    &self,
    tx_song: Sender<Result<Song, ScanError>>,
    tx_playlist: Sender<Result<Playlist, ScanError>>,
  ) -> Result<usize, ScanError> {
    self.check_dirs()?;

    let file_list = get_files_recursively(self.dir.clone())?;

    let mut len = 0;

    println!("{:?}", file_list.playlist_list);

    for playlist in file_list.playlist_list {
      let playlist_scan_res = self.scan_playlist(&playlist);
      if playlist_scan_res.is_err() {
        tx_playlist
          .send(Err(ScanError::String(format!(
            "Failed to scan {}: {:?}",
            playlist.display(),
            playlist_scan_res.unwrap_err()
          ))))
          .expect("channel will be there waiting for the pool");
        continue;
      }

      let (playlist_dets, songs) = playlist_scan_res.unwrap();
      tx_playlist
        .send(Ok(playlist_dets.clone()))
        .expect("channel will be there waiting for the pool");

      len += songs.len();

      for s in songs {
        self.scan_song_in_pool(tx_song.clone(), s);
      }
      continue;
    }

    drop(tx_song);
    drop(tx_playlist);

    Ok(len)
  }
}
