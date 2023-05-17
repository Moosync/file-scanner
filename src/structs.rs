use std::path::PathBuf;

#[derive(Default, Debug)]
#[napi(object)]
pub struct Song {
  pub bitrate: Option<u32>,
  pub sample_rate: Option<u32>,
  pub duration: Option<f64>,
  pub path: Option<String>,
  pub size: Option<u32>,
  pub title: Option<String>,
  pub album: Option<String>,
  pub artists: Option<String>,
  pub year: Option<String>,
  pub genre: Option<String>,
  pub lyrics: Option<String>,
  pub track_no: Option<String>,
  pub high_path: Option<String>,
  pub low_path: Option<String>,
  pub song_type: Option<String>,
  pub playlist_id: Option<String>,
}

pub struct FileList {
  pub file_list: Vec<(PathBuf, u64)>,
  pub playlist_list: Vec<PathBuf>,
}

#[derive(Default, Debug, Clone)]
#[napi(object)]
pub struct Playlist {
  pub id: String,
  pub title: String,
}
