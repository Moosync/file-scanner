use std::{cmp::min, collections::HashSet, path::PathBuf};

use sqlite3::{Connection, Error, State};

use crate::error::ScanError;

fn get_database(dir: PathBuf) -> Result<Connection, Error> {
  sqlite3::open(dir)
}

const EXPRESSION_LIMIT: usize = 998;

pub fn files_not_in_db(
  database: PathBuf,
  file_list: Vec<(PathBuf, u64)>,
) -> Result<Vec<(PathBuf, u64)>, ScanError> {
  let hash_set: HashSet<(PathBuf, u64)> = file_list.into_iter().collect();

  let statement_raw = format!(
    "SELECT path, size from allsongs WHERE {}",
    "(path = ? AND size = ?) OR ".repeat(min(hash_set.len(), EXPRESSION_LIMIT))
  );

  let mut statement = statement_raw.chars();

  statement.next_back();
  statement.next_back();
  statement.next_back();

  let mut result: HashSet<(PathBuf, u64)> = HashSet::new();

  let connection = get_database(database)?;

  let mut cursor = connection.prepare(statement.as_str())?;

  let mut i = 1;
  for (path, size) in hash_set.iter() {
    cursor.bind(
      i,
      dunce::canonicalize(path)?
        .to_string_lossy()
        .to_string()
        .as_str(),
    )?;
    cursor.bind(i + 1, *size as f64)?;

    i += 2;

    if i == (EXPRESSION_LIMIT * 2) + 1 {
      while let State::Row = cursor.next().unwrap() {
        let path = cursor.read::<String>(0)?;
        let size = cursor.read::<i64>(1)?;

        result.insert((PathBuf::from(path), size as u64));
      }

      i = 1;
      cursor = connection.prepare(statement.as_str())?;
      continue;
    }
  }

  while let State::Row = cursor.next().unwrap() {
    let path = cursor.read::<String>(0)?;
    let size = cursor.read::<i64>(1)?;

    result.insert((PathBuf::from(path), size as u64));
  }

  let diff = hash_set.difference(&result).clone();

  let vec = diff.into_iter().map(|v| v.clone()).collect();

  Ok(vec)
}
