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

pub struct PlayerApp {
    library: Library,
    am: AudioManager,
    alive: bool,
    active_song: Option<SongInfo>,
    playing_file_ix: usize,
    selected_file_ix: usize,
}

impl PlayerApp {
    pub fn new(root_dir: &Path) -> Result<Self> {
        Ok(Self {
            library: Library::new(root_dir)?,
            am: AudioManager::new()?,
            alive: true,
            active_song: None,
            playing_file_ix: 0,
            selected_file_ix: 0,
        })
    }

    pub fn library(&self) -> &Library {
        &self.library
    }

    pub fn audio_manager(&self) -> &AudioManager {
        &self.am
    }

    pub fn selected_file_ix(&self) -> usize {
        self.selected_file_ix
    }

    pub fn update(&mut self, dt: f64) -> Result<()> {
        self.am.update(dt);
        self.handle_events()?;
        if let Some(s) = &self.active_song {
            if self.am.playback_progress > s.duration
                && self.playing_file_ix < self.library().files().len() - 1
            {
                self.playing_file_ix += 1;
                self.play_at_ix()?;
            }
        }
        Ok(())
    }

    fn handle_events(&mut self) -> Result<()> {
        if event::poll(Duration::from_millis(16))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == event::KeyEventKind::Press {
                    if key.code == KeyCode::Char('q') {
                        self.alive = false;
                    } else if key.code == KeyCode::Char('p') {
                        if self.active_song.is_some() {
                            self.am.toggle_playback();
                        }
                    } else if key.code == KeyCode::Char('s') {
                        self.library.scan()?;
                    } else if key.code == KeyCode::Down {
                        self.selected_file_ix =
                            (self.selected_file_ix + 1).min(self.library().files().len() - 1);
                    } else if key.code == KeyCode::Up {
                        self.selected_file_ix = self.selected_file_ix.max(1) - 1;
                    } else if key.code == KeyCode::Enter {
                        self.playing_file_ix = self.selected_file_ix;
                        self.play_at_ix()?;
                    } else if key.code == KeyCode::Char('=') {
                        self.volume_up();
                    } else if key.code == KeyCode::Char('-') {
                        self.volume_down();
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
        let path = PathBuf::from(&self.library().files()[self.playing_file_ix].file_path);
        self.am.set_active_source(&path)?;
        self.active_song = Some(self.library().files()[self.playing_file_ix].clone());
        self.am.play();
        Ok(())
    }

    pub fn active_song(&self) -> Option<&SongInfo> {
        self.active_song.as_ref()
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

    pub fn play(&mut self) {
        self.sink.play();
    }

    pub fn pause(&mut self) {
        self.sink.pause();
    }

    pub fn update(&mut self, dt: f64) {
        if !self.sink.is_paused() {
            self.playback_progress += Duration::from_secs_f64(dt);
            if self.playback_progress > self.active_source_duration.unwrap_or(Duration::ZERO) {
                self.sink.pause();
            }
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
    _tags: Vec<Option<Box<dyn AudioTag + Send + Sync>>>,
}

impl Library {
    pub fn new(root_dir: &Path) -> Result<Self> {
        let mut s = Self {
            root_dir: root_dir.to_path_buf(),
            files: vec![],
            _tags: vec![],
        };
        s.scan()?;
        Ok(s)
    }

    pub fn files(&self) -> &[SongInfo] {
        &self.files
    }

    pub fn _tags(&self) -> &Vec<Option<Box<dyn AudioTag + Send + Sync>>> {
        &self._tags
    }

    pub fn scan(&mut self) -> Result<()> {
        self.files.clear();
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
        Ok(())
    }
}
