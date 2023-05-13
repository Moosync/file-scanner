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
use structs::Song;

use crate::scanner::start_scan;

#[napi(
  ts_args_type = "dir: string, thumbnailDir: string, databaseDir: string, callback: (err: null | Error, result: Song) => void"
)]
pub fn scan_files(
  dir: String,
  thumbnail_dir: String,
  database_dir: String,
  callback: JsFunction,
) -> Result<Undefined, napi::Error> {
  let thumbnail_dir = PathBuf::from_str(thumbnail_dir.as_str())?;
  let dir = PathBuf::from_str(dir.as_str())?;
  let database_dir = PathBuf::from_str(database_dir.as_str())?;

  let tsfn: ThreadsafeFunction<Song, ErrorStrategy::CalleeHandled> =
    callback.create_threadsafe_function(0, |ctx| Ok(vec![ctx.value]))?;

  spawn(move || {
    let (tx, rx) = channel();

    let pool = start_scan(dir, thumbnail_dir, database_dir, tx);
    if pool.is_err() {
      let cloned = tsfn.clone();
      cloned.call(
        Err(pool.err().unwrap().into()),
        ThreadsafeFunctionCallMode::Blocking,
      );
      return;
    }

    for received in rx {
      let cloned = tsfn.clone();
      cloned.call(
        received.map_err(|e| e.into()),
        ThreadsafeFunctionCallMode::NonBlocking,
      );
    }

    pool.unwrap().join();
  });

  Ok(())
}
