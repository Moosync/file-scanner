use std::{
  fmt::{self},
  num::ParseFloatError,
  time::SystemTimeError,
};

use fast_image_resize::{DifferentTypesOfPixelsError, ImageBufferError};
use image::ImageError;
use lofty::LoftyError;

#[derive(Debug)]
pub enum ScanError {
  IOError(std::io::Error),
  SystemTimeError(SystemTimeError),
  String(String),
  ImageBufferError(ImageBufferError),
  ImageError(ImageError),
  DifferentTypesOfPixelsError(DifferentTypesOfPixelsError),
  LoftyError(LoftyError),
  Sqlite3Error(sqlite3::Error),
  ParseFloatError(ParseFloatError),
  JWalkError(jwalk::Error),
}

impl fmt::Display for ScanError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      ScanError::IOError(e) => write!(f, "{:?}", e.to_string()),
      ScanError::SystemTimeError(e) => write!(f, "{}", e.to_string()),
      ScanError::String(e) => write!(f, "{:?}", e),
      ScanError::ImageBufferError(e) => write!(f, "{:?}", e),
      ScanError::ImageError(e) => write!(f, "{:?}", e),
      ScanError::DifferentTypesOfPixelsError(e) => write!(f, "{:?}", e),
      ScanError::LoftyError(e) => write!(f, "{:?}", e),
      ScanError::Sqlite3Error(e) => write!(f, "{:?}", e),
      ScanError::ParseFloatError(e) => write!(f, "{:?}", e),
      ScanError::JWalkError(e) => write!(f, "{:?}", e),
    }
  }
}

impl From<std::io::Error> for ScanError {
  fn from(value: std::io::Error) -> Self {
    ScanError::IOError(value)
  }
}

impl From<SystemTimeError> for ScanError {
  fn from(value: SystemTimeError) -> Self {
    ScanError::SystemTimeError(value)
  }
}

impl From<&str> for ScanError {
  fn from(value: &str) -> Self {
    ScanError::String(format!("{}", value))
  }
}

impl From<ImageBufferError> for ScanError {
  fn from(value: ImageBufferError) -> Self {
    ScanError::ImageBufferError(value)
  }
}

impl From<ImageError> for ScanError {
  fn from(value: ImageError) -> Self {
    ScanError::ImageError(value)
  }
}

impl From<DifferentTypesOfPixelsError> for ScanError {
  fn from(value: DifferentTypesOfPixelsError) -> Self {
    ScanError::DifferentTypesOfPixelsError(value)
  }
}

impl From<LoftyError> for ScanError {
  fn from(value: LoftyError) -> Self {
    ScanError::LoftyError(value)
  }
}

impl From<sqlite3::Error> for ScanError {
  fn from(value: sqlite3::Error) -> Self {
    ScanError::Sqlite3Error(value)
  }
}

impl From<ParseFloatError> for ScanError {
  fn from(value: ParseFloatError) -> Self {
    ScanError::ParseFloatError(value)
  }
}

impl From<jwalk::Error> for ScanError {
  fn from(value: jwalk::Error) -> Self {
    ScanError::JWalkError(value)
  }
}

impl Into<napi::Error> for ScanError {
  fn into(self) -> napi::Error {
    napi::Error::new(napi::Status::Unknown, self.to_string())
  }
}
