use std::io::{stdout, Stdout};

use crossterm::{
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use eyre::Result;
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Gauge, List, ListState, Paragraph},
    Frame, Terminal,
};

use crate::app::PlayerApp;

pub struct Tui {
    terminal: Terminal<CrosstermBackend<Stdout>>,
    ui_state: UiState,
}

impl Tui {
    pub fn new() -> Result<Self> {
        stdout().execute(EnterAlternateScreen)?;
        enable_raw_mode()?;
        let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;
        terminal.clear()?;
        Ok(Self {
            terminal,
            ui_state: UiState::new(),
        })
    }

    pub fn update(&mut self, app: &mut PlayerApp) -> Result<()> {
        self.ui_state
            .file_list_state
            .select(Some(app.selected_file_ix()));
        self.terminal
            .draw(|f| Self::ui(f, app, &mut self.ui_state))?;
        Ok(())
    }

    fn ui(frame: &mut Frame, app: &mut PlayerApp, ui_state: &mut UiState) {
        let layout =
            Layout::vertical([Constraint::Fill(8), Constraint::Min(1)]).split(frame.size());
        let bottom_layout =
            Layout::horizontal([Constraint::Fill(4), Constraint::Min(1)]).split(layout[1]);

        let tags = match app.library().tags().get(app.selected_file_ix()) {
            Some(Some(t)) => {
                format!(
                    "{}\n{}\n{}",
                    t.title().unwrap_or("Unknown Title"),
                    t.album_title().unwrap_or("Unknown Album"),
                    t.artist().unwrap_or("Unknown Artist")
                )
            }
            _ => String::from("Unknown Song"),
        };

        let tag_info = Paragraph::new(tags).block(
            Block::default()
                .title("Song Information")
                .borders(Borders::ALL),
        );
        frame.render_widget(tag_info, bottom_layout[1]);

        Self::draw_file_list(frame, app, ui_state, layout[0]);
        Self::draw_playback_bar(frame, app, ui_state, bottom_layout[0]);
    }

    fn draw_file_list(frame: &mut Frame, app: &mut PlayerApp, ui_state: &mut UiState, rect: Rect) {
        let file_paths = app
            .library()
            .files()
            .iter()
            .map(|f| f.to_str().unwrap_or("FAILED TO READ PATH"));

        let file_list = List::new(file_paths)
            .block(Block::default().title("File list").borders(Borders::ALL))
            .style(Style::default().fg(Color::White))
            .highlight_style(Style::default().add_modifier(Modifier::ITALIC))
            .highlight_symbol(">>")
            .repeat_highlight_symbol(true);

        frame.render_stateful_widget(file_list, rect, ui_state.file_list_state());
    }

    fn draw_playback_bar(
        frame: &mut Frame,
        app: &mut PlayerApp,
        _ui_state: &mut UiState,
        rect: Rect,
    ) {
        let playback_progress = match app.audio_manager().active_source_duration() {
            Some(v) => app.audio_manager().playback_progress().as_secs_f64() / v.as_secs_f64(),
            None => 0.0,
        };

        let playback_fmt = match app.audio_manager().active_source_duration() {
            Some(_) => {
                let minutes = app.audio_manager().playback_progress().as_secs() / 60;
                let secs = app.audio_manager().playback_progress().as_secs() % 60;
                format!("{:02}:{:02}", minutes, secs)
            }
            None => String::from("--:--"),
        };

        let total_fmt = match app.audio_manager().active_source_duration() {
            Some(v) => {
                let total_minutes = v.as_secs() / 60;
                let total_secs = v.as_secs() % 60;
                format!("{:02}:{:02}", total_minutes, total_secs)
            }
            None => String::from("--:--"),
        };

        let playback_bar = Gauge::default()
            .block(Block::default().borders(Borders::ALL).title("Playback"))
            .gauge_style(
                Style::default()
                    .fg(Color::White)
                    .bg(Color::Black)
                    .add_modifier(Modifier::BOLD),
            )
            .label(format!("{playback_fmt} / {total_fmt}",))
            .use_unicode(true)
            .ratio(playback_progress);
        frame.render_widget(playback_bar, rect);
    }
}

impl Drop for Tui {
    fn drop(&mut self) {
        if let Err(e) = stdout().execute(LeaveAlternateScreen) {
            eprintln!("Error executing LeaveAlternateScreen: {e}");
        }
        if let Err(e) = disable_raw_mode() {
            eprintln!("Error disabling raw mode: {e}")
        }
    }
}

struct UiState {
    file_list_state: ListState,
}

impl UiState {
    pub fn new() -> Self {
        Self {
            file_list_state: ListState::default(),
        }
    }

    pub fn file_list_state(&mut self) -> &mut ListState {
        &mut self.file_list_state
    }
}
