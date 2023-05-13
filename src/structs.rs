use std::path::PathBuf;

#[derive(Default, Debug)]
#[napi(object)]
pub struct Song {
  pub bitrate: Option<u32>,
  pub sample_rate: Option<u32>,
  pub duration: Option<u32>,
  pub path: String,
  pub size: u32,
  pub title: String,
  pub album: String,
  pub artists: String,
  pub year: String,
  pub genre: String,
  pub lyrics: String,
  pub track_no: String,
  pub high_path: Option<String>,
  pub low_path: Option<String>,
}

pub struct FileList {
  pub file_list: Vec<(PathBuf, u64)>,
  pub playlist_list: Vec<PathBuf>,
}
