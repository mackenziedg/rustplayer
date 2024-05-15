use std::io::BufReader;
use std::path::PathBuf;
use std::time::Instant;
use std::{fs::File, time::Duration};

use crossterm::event::{self, Event, KeyCode};
use rodio::{source::Source, Decoder, OutputStream, OutputStreamHandle, Sink};

use eyre::Result;

pub struct PlayerApp {
    library: Library,
    am: AudioManager,
    alive: bool,
    selected_file_ix: usize,
}

impl PlayerApp {
    pub fn new() -> Result<Self> {
        Ok(Self {
            library: Library::new(),
            am: AudioManager::new()?,
            alive: true,
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
        Ok(())
    }

    fn handle_events(&mut self) -> Result<()> {
        if event::poll(Duration::from_millis(16))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == event::KeyEventKind::Press {
                    if key.code == KeyCode::Char('q') {
                        self.alive = false;
                    } else if key.code == KeyCode::Char('p') {
                        self.am.toggle_playback();
                    } else if key.code == KeyCode::Char('s') {
                        self.library
                            .scan(&PathBuf::from("/home/mac/Downloads/test_music"))?;
                    } else if key.code == KeyCode::Down {
                        self.selected_file_ix =
                            (self.selected_file_ix + 1).min(self.library().files().len() - 1);
                    } else if key.code == KeyCode::Up {
                        self.selected_file_ix = (self.selected_file_ix - 1).max(0);
                    } else if key.code == KeyCode::Enter {
                        let f = BufReader::new(File::open(
                            &self.library().files()[self.selected_file_ix],
                        )?);
                        self.am.set_active_source(f)?;
                        self.am.play();
                    }
                }
            }
        }
        Ok(())
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
    stream: OutputStream,
    stream_handle: OutputStreamHandle,
    playback_progress: Duration,
    active_source_duration: Option<Duration>,
}

impl AudioManager {
    pub fn new() -> Result<Self> {
        let (stream, stream_handle) = OutputStream::try_default()?;
        let sink = Sink::try_new(&stream_handle)?;

        Ok(Self {
            sink,
            stream,
            stream_handle,
            playback_progress: Duration::ZERO,
            active_source_duration: None,
        })
    }

    pub fn toggle_playback(&mut self) {
        if self.sink.is_paused() {
            self.sink.play();
        } else {
            self.sink.pause();
        }
    }

    pub fn set_active_source(&mut self, file: BufReader<File>) -> Result<()> {
        let source = Decoder::new(file)?;
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
        }
    }

    pub fn set_volume(&mut self, volume: f32) {
        self.sink.set_volume(volume);
    }

    pub fn playback_progress(&self) -> &Duration {
        &self.playback_progress
    }

    pub fn active_source_duration(&self) -> Option<Duration> {
        self.active_source_duration
    }
}

pub struct Library {
    files: Vec<PathBuf>,
}

impl Library {
    pub fn new() -> Self {
        Self { files: vec![] }
    }

    pub fn files(&self) -> &[PathBuf] {
        &self.files
    }

    pub fn scan(&mut self, directory: &PathBuf) -> Result<()> {
        self.files.clear();
        let audio_files = std::fs::read_dir(directory)?
            .filter_map(|d| match d {
                Ok(p) => match p.file_type() {
                    Ok(_) => Some(p.path()),
                    Err(_) => None,
                },
                Err(_) => None,
            })
            .filter(|p| {
                p.extension()
                    .is_some_and(|e| ["mp3", "flac"].contains(&e.to_str().unwrap_or("")))
            });
        self.files.extend(audio_files);
        Ok(())
    }
}
