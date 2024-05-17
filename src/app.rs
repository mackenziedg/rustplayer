use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::{fs::File, time::Duration};

use audiotags::{AudioTag, Tag};
use crossterm::event::{self, Event, KeyCode};
use rodio::{source::Source, Decoder, OutputStream, OutputStreamHandle, Sink};

use eyre::Result;

#[derive(Debug, Clone)]
pub struct SongInfo {
    title: Option<String>,
    album: Option<String>,
    artist: Option<String>,
    _album_artist: Option<String>,
    _year: Option<i32>,
    _genre: Option<String>,
    track: (Option<u16>, Option<u16>),
    _disc: (Option<u16>, Option<u16>),
    duration: Duration,
    file_path: PathBuf,
}

impl SongInfo {
    fn new(path: &Path, tag: Box<dyn AudioTag>) -> Self {
        // If the file has the duration in the tags, great!
        // If not, we call ffmpeg/ffprobe to get the info
        let duration = match tag.duration() {
            Some(v) => Duration::from_secs_f64(v),
            None => mp3_duration::from_path(path).unwrap_or(Duration::ZERO),
        };

        Self {
            title: tag.title().map(|s| s.to_owned()),
            album: tag.album_title().map(|s| s.to_owned()),
            artist: tag.artist().map(|s| s.to_owned()),
            _album_artist: tag.album_artist().map(|s| s.to_owned()),
            _year: tag.year(),
            _genre: tag.genre().map(|s| s.to_owned()),
            track: tag.track(),
            _disc: tag.disc(),
            duration,
            file_path: path.to_path_buf(),
        }
    }

    pub fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    pub fn album(&self) -> Option<&str> {
        self.album.as_deref()
    }

    pub fn artist(&self) -> Option<&str> {
        self.artist.as_deref()
    }

    pub fn _album_artist(&self) -> Option<&str> {
        self._album_artist.as_deref()
    }

    pub fn _year(&self) -> &Option<i32> {
        &self._year
    }

    pub fn _genre(&self) -> Option<&str> {
        self._genre.as_deref()
    }

    pub fn track(&self) -> &(Option<u16>, Option<u16>) {
        &self.track
    }

    pub fn _disc(&self) -> &(Option<u16>, Option<u16>) {
        &self._disc
    }

    pub fn duration(&self) -> &Duration {
        &self.duration
    }

    pub fn _file_path(&self) -> &Path {
        &self.file_path
    }
}

#[derive(PartialEq)]
pub enum AppUiMode {
    FileList,
    SearchPopup,
    InfoPopup,
}

pub struct AppState {
    active_song: Option<SongInfo>,
    playing_file_ix: usize,
    selected_file_ix: usize,
    search_query: Option<String>,
    ui_mode: AppUiMode,
}

pub struct PlayerApp {
    library: Library,
    am: AudioManager,
    alive: bool,
    app_state: AppState,
}

impl PlayerApp {
    pub fn new(root_dir: &Path) -> Result<Self> {
        Ok(Self {
            library: Library::new(root_dir).with_scan()?,
            am: AudioManager::new()?,
            alive: true,
            app_state: AppState {
                active_song: None,
                playing_file_ix: 0,
                selected_file_ix: 0,
                search_query: None,
                ui_mode: AppUiMode::FileList,
            },
        })
    }

    pub fn library(&self) -> &Library {
        &self.library
    }

    pub fn audio_manager(&self) -> &AudioManager {
        &self.am
    }

    pub fn ui_mode(&self) -> &AppUiMode {
        &self.app_state.ui_mode
    }

    pub fn search_query(&self) -> Option<&str> {
        self.app_state.search_query.as_deref()
    }

    pub fn selected_file_ix(&self) -> usize {
        self.app_state.selected_file_ix
    }

    pub fn update(&mut self, dt: f64) -> Result<()> {
        self.am.update(dt);
        self.handle_events()?;
        if let Some(s) = &self.app_state.active_song {
            if self.am.playback_progress >= s.duration {
                if self.app_state.playing_file_ix < self.library().files().len() - 1 {
                    self.app_state.playing_file_ix += 1;
                    self.play_at_ix()?;
                } else {
                    self.am.pause();
                }
            }
        }
        Ok(())
    }

    fn handle_events(&mut self) -> Result<()> {
        if event::poll(Duration::from_millis(16))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == event::KeyEventKind::Press {
                    if self.app_state.ui_mode == AppUiMode::FileList {
                        if key.code == KeyCode::Char('q') {
                            self.alive = false;
                        } else if key.code == KeyCode::Char('p') {
                            if self.app_state.active_song.is_some() {
                                self.am.toggle_playback();
                            }
                        } else if key.code == KeyCode::Char('s') {
                            self.library.scan()?;
                        } else if key.code == KeyCode::Down {
                            self.app_state.selected_file_ix = (self.app_state.selected_file_ix + 1)
                                .min(self.library().files().len() - 1);
                        } else if key.code == KeyCode::Up {
                            self.app_state.selected_file_ix =
                                self.app_state.selected_file_ix.max(1) - 1;
                        } else if key.code == KeyCode::Right {
                            if self.app_state.active_song.is_some() {
                                if key.modifiers == crossterm::event::KeyModifiers::SHIFT {
                                    self.am.skip()
                                } else {
                                    self.am.seek_forward();
                                }
                            }
                        } else if key.code == KeyCode::Left {
                            if self.app_state.active_song.is_some() {
                                self.am.seek_backward();
                            }
                        } else if key.code == KeyCode::Enter {
                            self.app_state.playing_file_ix = self.app_state.selected_file_ix;
                            self.play_at_ix()?;
                        } else if key.code == KeyCode::Char('=') {
                            self.volume_up();
                        } else if key.code == KeyCode::Char('-') {
                            self.volume_down();
                        } else if key.code == KeyCode::Char('/') {
                            self.app_state.ui_mode = AppUiMode::SearchPopup;
                        }
                    } else if self.app_state.ui_mode == AppUiMode::SearchPopup {
                        if key.code == KeyCode::Enter {
                            self.app_state.ui_mode = AppUiMode::FileList;
                        } else if key.code == KeyCode::Backspace {
                            if let Some(q) = &self.app_state.search_query {
                                if q.len() == 1 {
                                    self.app_state.search_query = None;
                                } else {
                                    self.app_state.search_query =
                                        Some(q[..q.len() - 1].to_string());
                                }
                            }
                        } else if let KeyCode::Char(c) = key.code {
                            let mut query = self.app_state.search_query.clone();
                            if self.app_state.search_query.is_none() {
                                query = Some(c.to_string());
                            } else {
                                query.as_mut().unwrap().push(c);
                            }
                            self.app_state.search_query = query;
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn volume_up(&mut self) {
        self.am.set_volume((self.am.get_volume() + 0.01).min(1.0))
    }

    fn volume_down(&mut self) {
        self.am.set_volume((self.am.get_volume() - 0.01).max(0.0))
    }

    pub fn volume(&self) -> f32 {
        self.am.get_volume()
    }

    fn play_at_ix(&mut self) -> Result<()> {
        let path = PathBuf::from(&self.library().files()[self.app_state.playing_file_ix].file_path);
        self.am.set_active_source(&path)?;
        self.app_state.active_song =
            Some(self.library().files()[self.app_state.playing_file_ix].clone());
        self.am.play();
        Ok(())
    }

    pub fn active_song(&self) -> Option<&SongInfo> {
        self.app_state.active_song.as_ref()
    }

    pub fn is_alive(&self) -> bool {
        self.alive
    }

    pub fn is_playing(&self) -> bool {
        !self.am.sink.is_paused()
    }
}

pub struct AudioManager {
    sink: Sink,
    _stream: OutputStream,
    _stream_handle: OutputStreamHandle,
    playback_progress: Duration,
    active_source_duration: Option<Duration>,
}

impl AudioManager {
    pub fn new() -> Result<Self> {
        let (stream, stream_handle) = OutputStream::try_default()?;
        let sink = Sink::try_new(&stream_handle)?;
        sink.pause();

        Ok(Self {
            sink,
            _stream: stream,
            _stream_handle: stream_handle,
            playback_progress: Duration::ZERO,
            active_source_duration: None,
        })
    }

    pub fn toggle_playback(&mut self) {
        if self.sink.is_paused() {
            self.play();
        } else {
            self.pause();
        }
    }

    pub fn set_active_source(&mut self, path: &PathBuf) -> Result<()> {
        let source = Decoder::new(BufReader::new(File::open(path)?))?;
        self.active_source_duration = source.total_duration();
        self.sink.clear();
        self.sink.append(source);
        self.playback_progress = Duration::ZERO;
        Ok(())
    }

    pub fn skip(&mut self) {
        self.playback_progress = self
            .active_source_duration
            .expect("Already checked if we have an active source.");
    }

    pub fn seek_forward(&mut self) {
        let seek_diff = Duration::from_secs(5);
        if let Ok(()) = self.sink.try_seek(self.playback_progress + seek_diff) {
            self.playback_progress += seek_diff;
        }
    }

    pub fn seek_backward(&mut self) {
        let seek_diff = Duration::from_secs(1);
        if seek_diff > self.playback_progress {
            self.playback_progress = Duration::ZERO;
            let _ = self.sink.try_seek(self.playback_progress);
        } else if let Ok(()) = self.sink.try_seek(self.playback_progress - seek_diff) {
            self.playback_progress -= seek_diff;
        }
    }

    pub fn play(&mut self) {
        self.sink.play();
    }

    pub fn pause(&mut self) {
        self.sink.pause();
    }

    pub fn update(&mut self, dt: f64) {
        if !self.sink.is_paused() {
            self.playback_progress += Duration::from_secs_f64(dt);
        }
    }

    pub fn get_volume(&self) -> f32 {
        self.sink.volume()
    }

    pub fn set_volume(&mut self, volume: f32) {
        self.sink.set_volume(volume);
    }

    pub fn playback_progress(&self) -> &Duration {
        &self.playback_progress
    }

    pub fn _active_source_duration(&self) -> Option<Duration> {
        self.active_source_duration
    }
}

pub struct Library {
    root_dir: PathBuf,
    files: Vec<SongInfo>,
}

impl Library {
    pub fn new(root_dir: &Path) -> Self {
        Self {
            root_dir: root_dir.to_path_buf(),
            files: vec![],
        }
    }

    pub fn with_scan(mut self) -> Result<Self> {
        let _ = self.scan()?;
        Ok(self)
    }

    pub fn files(&self) -> &[SongInfo] {
        &self.files
    }

    /// Scan [`Self::root_dir`] for audio files.
    ///
    /// If successful, returns a [`Result`] containing the number of total files scanned.
    /// The number of files successfully loaded is just the size of [`Self::files`].
    pub fn scan(&mut self) -> Result<usize> {
        self.files.clear();
        let mut total_files_seen = 0usize;
        let mut to_scan = vec![self.root_dir.to_path_buf()];
        while let Some(dir) = to_scan.pop() {
            for p in std::fs::read_dir(dir)?.flatten() {
                let path = p.path();
                if p.file_type()?.is_dir() {
                    to_scan.push(path);
                } else if p.file_type()?.is_file()
                    && p.path()
                        .extension()
                        .is_some_and(|e| ["mp3", "flac"].contains(&e.to_str().unwrap_or("")))
                {
                    total_files_seen += 1;
                    let tag = match Tag::new().read_from_path(&p.path()) {
                        Ok(t) => t,
                        Err(_) => continue,
                    };
                    self.files.push(SongInfo::new(&p.path(), tag));
                }
            }
        }

        self.files.sort_by_key(|f| {
            (
                f.artist.clone().unwrap_or("Unknown".to_string()),
                f.album.clone().unwrap_or("Unknown".to_string()),
                f.track.0.unwrap_or(0),
            )
        });
        Ok(total_files_seen)
    }
}

#[cfg(test)]
mod tests {
    extern crate tempdir;

    use std::fs::{create_dir, File};
    use tempdir::TempDir;

    use super::*;

    #[test]
    fn test_library_scans_empty_dir() {
        let td = TempDir::new("tempdir").unwrap();
        let l = Library::new(td.path());
        assert!(l.files().is_empty());
    }

    #[test]
    fn test_library_scans_flat_dir_no_valid_files() {
        let td = TempDir::new("tempdir").unwrap();
        let file_path = td.path().join("test_file.mp3");
        let _file = File::create(file_path).unwrap();
        let mut l = Library::new(td.path());
        assert_eq!(l.scan().unwrap(), 1);
        // We don't add files unless they can be parsed as valid mp3/flac
        assert!(l.files().is_empty());
    }

    #[test]
    fn test_library_scans_nested_dir_no_valid_files() {
        let td = TempDir::new("tempdir").unwrap();
        create_dir(td.path().join("subdir_1")).unwrap();
        let file_path = td.path().join("subdir_1").join("test_file.mp3");
        let _file = File::create(file_path).unwrap();
        create_dir(td.path().join("subdir_2")).unwrap();
        let file_path = td.path().join("subdir_2").join("test_file.mp3");
        let _file = File::create(file_path).unwrap();
        create_dir(td.path().join("subdir_3")).unwrap();
        let file_path = td.path().join("subdir_3").join("test_file.mp3");
        let _file = File::create(file_path).unwrap();
        create_dir(td.path().join("subdir_1").join("subsubdir_1")).unwrap();
        let file_path = td
            .path()
            .join("subdir_1")
            .join("subsubdir_1")
            .join("test_file.mp3");
        let _file = File::create(file_path).unwrap();
        let mut l = Library::new(td.path());
        assert_eq!(l.scan().unwrap(), 4);
        // We don't add files unless they can be parsed as valid mp3/flac
        assert!(l.files().is_empty());
    }

    #[test]
    fn test_audio_manager_toggle_playback() {
        let mut am = AudioManager::new().unwrap();
        assert!(am.sink.is_paused());
        am.toggle_playback();
        assert!(!am.sink.is_paused());
        am.toggle_playback();
        assert!(am.sink.is_paused());
        am.play();
        assert!(!am.sink.is_paused());
        am.play();
        assert!(!am.sink.is_paused());
        am.pause();
        assert!(am.sink.is_paused());
        am.pause();
        assert!(am.sink.is_paused());
    }
}
