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
  tx_song: Sender<Result<Song, ScanError>>,
  database_path: PathBuf,
  thumbnail_dir: PathBuf,
}

impl<'a> SongScanner<'a> {
  pub fn new(
    dir: PathBuf,
    pool: &'a mut ThreadPool,
    tx_song: Sender<Result<Song, ScanError>>,
    database_path: PathBuf,
    thumbnail_dir: PathBuf,
  ) -> Self {
    Self {
      dir,
      pool,
      tx_song,
      database_path,
      thumbnail_dir,
    }
  }

  fn check_dirs(&self) -> Result<(), ScanError> {
    check_directory(self.dir.clone())?;
    check_directory(self.thumbnail_dir.clone())?;

    Ok(())
  }

  pub fn scan_in_pool(&self, size: u64, path: PathBuf) {
    let tx = self.tx_song.clone();
    let thumbnail_dir = self.thumbnail_dir.clone();
    self.pool.execute(move || {
      let mut metadata = scan_file(&path, &thumbnail_dir, &None, size, false);
      if metadata.is_err() {
        println!("Guessing filetype");
        metadata = scan_file(&path, &thumbnail_dir, &None, size, true);
      }

      tx.send(metadata)
        .expect("channel will be there waiting for the pool");
    });
  }

  pub fn start(&self) -> Result<(), ScanError> {
    self.check_dirs()?;

    let file_list = get_files_recursively(self.dir.clone())?;
    let song_list = files_not_in_db(self.database_path.clone(), file_list.file_list).unwrap();

    for (file_path, size) in song_list {
      self.scan_in_pool(size, file_path);
    }

    Ok(())
  }
}