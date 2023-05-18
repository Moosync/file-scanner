#![deny(clippy::all)]

#[macro_use]
extern crate napi_derive;

mod database;
mod error;
mod playlist_scanner;
mod song_scanner;
mod structs;
mod utils;

use std::{path::PathBuf, str::FromStr, sync::mpsc::channel, thread::spawn};

use napi::{
  bindgen_prelude::Undefined,
  threadsafe_function::{ErrorStrategy, ThreadsafeFunction, ThreadsafeFunctionCallMode},
  JsFunction,
};
use playlist_scanner::PlaylistScanner;
use song_scanner::SongScanner;
use structs::{Playlist, Song};
use threadpool::ThreadPool;

#[napi(
  ts_args_type = "dir: string, thumbnailDir: string, databaseDir: string, artistSplit: string, threads: number, force: boolean, callback_song: (err: null | Error, result: Song) => void, callback_playlist: (err: null | Error, result: Playlist) => void"
)]
pub fn scan_files(
  dir: String,
  thumbnail_dir: String,
  database_dir: String,
  artist_split: String,
  threads: i32,
  force: bool,
  callback_songs: JsFunction,
  callback_playlists: JsFunction,
) -> Result<Undefined, napi::Error> {
  let thumbnail_dir = PathBuf::from_str(thumbnail_dir.as_str())?;
  let dir = PathBuf::from_str(dir.as_str())?;
  let database_dir = PathBuf::from_str(database_dir.as_str())?;

  let tsfn_songs: ThreadsafeFunction<Song, ErrorStrategy::CalleeHandled> =
    callback_songs.create_threadsafe_function(0, |ctx| Ok(vec![ctx.value]))?;

  let tsfn_playlists: ThreadsafeFunction<Playlist, ErrorStrategy::CalleeHandled> =
    callback_playlists.create_threadsafe_function(0, |ctx| Ok(vec![ctx.value]))?;

  spawn(move || {
    let (tx_song, rx_song) = channel();
    let (tx_playlist, rx_playlist) = channel();

    let cpus = num_cpus::get();
    let thread_count = if threads <= 0 {
      cpus
    } else if threads as usize > cpus {
      cpus
    } else {
      threads as usize
    };

    let mut pool = ThreadPool::new(thread_count);
    let song_scanner = SongScanner::new(
      dir.clone(),
      &mut pool,
      database_dir.clone(),
      thumbnail_dir.clone(),
      artist_split,
    );

    let res = song_scanner.start(tx_song.clone(), force);
    if res.is_err() {
      let cloned = tsfn_songs.clone();
      cloned.call(
        Err(res.err().unwrap().into()),
        ThreadsafeFunctionCallMode::Blocking,
      );
      return;
    }

    let playlist_scanner = PlaylistScanner::new(dir, thumbnail_dir, song_scanner);
    let res1 = playlist_scanner.start(tx_song, tx_playlist);
    if res1.is_err() {
      let cloned = tsfn_songs.clone();
      cloned.call(
        Err(res.err().unwrap().into()),
        ThreadsafeFunctionCallMode::Blocking,
      );
      return;
    }

    let mut song_ended = false;
    let mut playlist_ended = false;
    loop {
      let song = rx_song.try_recv();
      if song.is_err() {
        if song
          .unwrap_err()
          .eq(&std::sync::mpsc::TryRecvError::Disconnected)
        {
          song_ended = true;
        }
      } else {
        let cloned = tsfn_songs.clone();
        cloned.call(
          song.unwrap().map_err(|e| e.into()),
          ThreadsafeFunctionCallMode::NonBlocking,
        );
      }

      let playlist = rx_playlist.try_recv();
      if playlist.is_err() {
        if playlist
          .unwrap_err()
          .eq(&std::sync::mpsc::TryRecvError::Disconnected)
        {
          playlist_ended = true;
        }
      } else {
        let cloned = tsfn_playlists.clone();
        cloned.call(
          playlist.unwrap().map_err(|e| e.into()),
          ThreadsafeFunctionCallMode::NonBlocking,
        );
      }

      if song_ended && playlist_ended {
        break;
      }
    }

    drop(playlist_scanner);
    pool.join();
  });

  Ok(())
}
