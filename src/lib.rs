#![deny(clippy::all)]

#[macro_use]
extern crate napi_derive;

mod database;
mod error;
mod scanner;
mod structs;

use std::{path::PathBuf, str::FromStr, sync::mpsc::channel, thread::spawn};

use napi::{
  bindgen_prelude::Undefined,
  threadsafe_function::{ErrorStrategy, ThreadsafeFunction, ThreadsafeFunctionCallMode},
  JsFunction,
};
use structs::{Playlist, Song};
use threadpool::ThreadPool;

use crate::scanner::start_scan;

#[napi(
  ts_args_type = "dir: string, thumbnailDir: string, databaseDir: string, callback: (err: null | Error, result: Song) => void, callback: (err: null | Error, result: Playlist) => void"
)]
pub fn scan_files(
  dir: String,
  thumbnail_dir: String,
  database_dir: String,
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

    let pool = &ThreadPool::new(num_cpus::get());

    let pool = start_scan(dir, thumbnail_dir, database_dir, tx_song, tx_playlist, pool);
    if pool.is_err() {
      let cloned = tsfn_songs.clone();
      cloned.call(
        Err(pool.err().unwrap().into()),
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

    // for received in rx_song {
    //   let cloned = tsfn.clone();
    //   cloned.call(
    //     received.map_err(|e| e.into()),
    //     ThreadsafeFunctionCallMode::NonBlocking,
    //   );
    // }

    pool.unwrap().join();
  });

  Ok(())
}
