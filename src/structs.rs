use std::path::PathBuf;

#[derive(Default, Debug)]
#[napi(object)]
pub struct Song {
  pub _id: String,
  pub bitrate: Option<u32>,
  pub sample_rate: Option<u32>,
  pub duration: Option<f64>,
  pub path: Option<String>,
  pub size: Option<u32>,
  pub title: Option<String>,
  pub album: Option<Album>,
  pub artists: Vec<Artists>,
  pub year: Option<String>,
  pub genre: Option<Vec<String>>,
  pub lyrics: Option<String>,
  pub track_no: Option<String>,

  #[napi(js_name = "song_coverPath_high")]
  pub high_path: Option<String>,

  #[napi(js_name = "song_coverPath_low")]
  pub low_path: Option<String>,

  #[napi(js_name = "type")]
  pub song_type: String,
  pub playlist_id: Option<String>,
}

#[derive(Default, Debug)]
#[napi(object)]
pub struct Album {
  #[napi(js_name = "album_id")]
  pub album_id: String,

  #[napi(js_name = "album_name")]
  pub album_name: String,

  #[napi(js_name = "album_coverPath_high")]
  pub album_cover_path_high: Option<String>,

  #[napi(js_name = "album_coverPath_low")]
  pub album_cover_path_low: Option<String>,
}

#[derive(Default, Debug)]
#[napi(object)]
pub struct Artists {
  #[napi(js_name = "artist_id")]
  pub artist_id: String,

  #[napi(js_name = "artist_name")]
  pub artist_name: String,
}

#[derive(Debug)]
pub struct FileList {
  pub file_list: Vec<(PathBuf, u64)>,
  pub playlist_list: Vec<PathBuf>,
}

#[derive(Default, Debug, Clone)]
#[napi(object)]
pub struct Playlist {
  pub id: String,
  pub title: String,
  pub path: String,
}

#[derive(Debug)]
#[napi(object)]
pub struct SongWithLen {
  pub song: Song,
  pub size: u32,
  pub current: u32,
}
