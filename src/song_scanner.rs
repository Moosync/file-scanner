use std::{path::PathBuf, sync::mpsc::Sender};

use threadpool::ThreadPool;

use crate::{
  database::files_not_in_db,
  error::ScanError,
  structs::Song,
  utils::{check_directory, get_files_recursively, scan_file},
};

pub struct SongScanner<'a> {
  dir: PathBuf,
  pool: &'a mut ThreadPool,
  database_path: PathBuf,
  thumbnail_dir: PathBuf,
  artist_split: String,
}

impl<'a> SongScanner<'a> {
  pub fn new(
    dir: PathBuf,
    pool: &'a mut ThreadPool,
    database_path: PathBuf,
    thumbnail_dir: PathBuf,
    artist_split: String,
  ) -> Self {
    Self {
      dir,
      pool,
      database_path,
      thumbnail_dir,
      artist_split,
    }
  }

  fn check_dirs(&self) -> Result<(), ScanError> {
    check_directory(self.thumbnail_dir.clone())?;

    Ok(())
  }

  pub fn scan_in_pool(
    &self,
    tx: Sender<Result<Song, ScanError>>,
    size: u64,
    path: PathBuf,
    playlist_id: Option<String>,
  ) {
    let thumbnail_dir = self.thumbnail_dir.clone();
    let artist_split = self.artist_split.clone();
    self.pool.execute(move || {
      let mut metadata = scan_file(
        &path,
        &thumbnail_dir,
        &playlist_id,
        size,
        false,
        &artist_split,
      );
      if metadata.is_err() {
        metadata = scan_file(&path, &thumbnail_dir, &None, size, true, &artist_split);
      }

      tx.send(metadata)
        .expect("channel will be there waiting for the pool");
    });
  }

  pub fn start(
    &self,
    tx_song: Sender<Result<Song, ScanError>>,
    force: bool,
  ) -> Result<usize, ScanError> {
    self.check_dirs()?;

    let file_list = get_files_recursively(self.dir.clone())?;

    let song_list = if !force {
      files_not_in_db(self.database_path.clone(), file_list.file_list)?
    } else {
      file_list.file_list
    };

    println!("{:?}", song_list);

    let len = song_list.len();

    for (file_path, size) in song_list {
      self.scan_in_pool(tx_song.clone(), size, file_path, None);
    }

    drop(tx_song);

    Ok(len)
  }
}
