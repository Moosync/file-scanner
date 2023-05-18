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
    check_directory(self.thumbnail_dir.clone())?;

    Ok(())
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
          let path = PathBuf::from_str(line.as_str());
          if let Ok(path_parsed) = path {
            if path_parsed.exists() {
              let metadata = fs::metadata(&path_parsed)?;
              let mut song = Song::default();
              song.path = Some(line);
              song.artists = self.parse_artists(artists);
              song.duration = duration;
              song.title = title;
              song.song_type = song_type;
              song.size = Some(metadata.len() as u32);
              songs.push(song);
            }
          }

          artists = None;
          duration = None;
          title = None;
          song_type = None;
        }
      }
    }

    Ok((
      Playlist {
        id: Uuid::new_v4().to_string(),
        title: playlist_title,
      },
      songs,
    ))
  }

  fn scan_song_in_pool(&self, tx_song: Sender<Result<Song, ScanError>>, s: Song) {
    if s.song_type.is_none() || s.song_type.is_some() && s.song_type.unwrap() == "LOCAL" {
      self.song_scanner.scan_in_pool(
        tx_song,
        s.size.unwrap() as u64,
        PathBuf::from_str(s.path.unwrap().as_str()).unwrap(),
      )
    }
  }

  pub fn start(
    &self,
    tx_song: Sender<Result<Song, ScanError>>,
    tx_playlist: Sender<Result<Playlist, ScanError>>,
  ) -> Result<(), ScanError> {
    self.check_dirs()?;

    let file_list = get_files_recursively(self.dir.clone())?;

    for playlist in file_list.playlist_list {
      let (playlist_dets, songs) = self.scan_playlist(&playlist)?;
      tx_playlist
        .send(Ok(playlist_dets.clone()))
        .expect("channel will be there waiting for the pool");

      for s in songs {
        self.scan_song_in_pool(tx_song.clone(), s);
      }
    }

    drop(tx_song);
    drop(tx_playlist);

    Ok(())
  }
}
