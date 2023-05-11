use std::{collections::HashSet, path::PathBuf};

use sqlite3::{Connection, Error, State};

use crate::error::ScanError;

fn get_database(dir: PathBuf) -> Result<Connection, Error> {
  sqlite3::open(dir)
}

pub fn files_not_in_db(
  database: PathBuf,
  file_list: Vec<(PathBuf, u64)>,
) -> Result<Vec<(PathBuf, u64)>, ScanError> {
  let hash_set: HashSet<(PathBuf, u64)> = file_list.into_iter().collect();

  let statement_raw = format!(
    "SELECT path, size from allsongs WHERE {}",
    "(path = ? AND size = ?) OR ".repeat(hash_set.len())
  );

  let mut statement = statement_raw.chars();

  statement.next_back();
  statement.next_back();
  statement.next_back();

  println!("{}", statement.as_str());

  let mut result: HashSet<(PathBuf, u64)> = HashSet::new();

  let connection = get_database(database)?;

  let mut cursor = connection.prepare(statement.as_str())?;

  let mut i = 1;
  for (path, size) in hash_set.iter() {
    cursor.bind(i, path.to_str().unwrap())?;
    cursor.bind(i + 1, *size as f64)?;
    i += 2;
  }

  while let State::Row = cursor.next().unwrap() {
    let path = cursor.read::<String>(0).unwrap();
    let size = cursor.read::<i64>(1).unwrap();

    result.insert((PathBuf::from(path), size as u64));
  }

  let diff = hash_set.difference(&result).clone();

  let vec = diff.into_iter().map(|v| v.clone()).collect();

  Ok(vec)
}
