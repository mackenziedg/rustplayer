use std::io::{stdout, Stdout};

use crossterm::{
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use eyre::Result;
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    widgets::{
        block::{Position, Title},
        Block, Borders, Gauge, Paragraph, Row, Table, TableState,
    },
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
            .table_state
            .select(Some(app.selected_file_ix()));
        self.terminal
            .draw(|f| Self::ui(f, app, &mut self.ui_state))?;
        Ok(())
    }

    fn ui(frame: &mut Frame, app: &mut PlayerApp, ui_state: &mut UiState) {
        let layout =
            Layout::vertical([Constraint::Fill(8), Constraint::Min(3)]).split(frame.size());
        let bottom_layout =
            Layout::horizontal([Constraint::Fill(4), Constraint::Min(1)]).split(layout[1]);

        let tags = match app.active_song() {
            Some(t) => {
                format!(
                    "{}\n{}\n{}",
                    t.title().unwrap_or("Unknown Title"),
                    t.album().unwrap_or("Unknown Album"),
                    t.artist().unwrap_or("Unknown Artist")
                )
            }
            _ => String::from("Unknown Song"),
        };

        let tag_info =
            Paragraph::new(tags).block(Block::default().title("Now Playing").borders(Borders::ALL));
        frame.render_widget(tag_info, bottom_layout[1]);

        Self::draw_file_list(frame, app, ui_state, layout[0]);
        Self::draw_playback_bar(frame, app, ui_state, bottom_layout[0]);
    }

    fn draw_file_list(frame: &mut Frame, app: &mut PlayerApp, ui_state: &mut UiState, rect: Rect) {
        let table_rows = app
            .library()
            .files()
            .iter()
            .map(|s| {
                Row::new(vec![
                    format!("{:02}", s.track().0.unwrap_or(0)),     // Track ID
                    format!("{}", s.title().unwrap_or("Unknown")),  // Song title
                    format!("{}", s.artist().unwrap_or("Unknown")), // Artist name
                    format!("{}", s.album().unwrap_or("Unknown")),  // Album name
                    format!(
                        "{}",
                        format!(
                            "{:02}:{:02}",
                            s.duration().as_secs() / 60,
                            s.duration().as_secs() % 60
                        )
                    ), // Duration
                ])
            })
            .collect::<Vec<_>>();
        let widths = [
            Constraint::Fill(1), // Track ID
            Constraint::Fill(5), // Song title
            Constraint::Fill(5), // Artist name
            Constraint::Fill(5), // Album name
            Constraint::Fill(2), // Length
        ];
        let header =
            Row::new(["#", "Title", "Artist", "Album", "Length"]).style(Style::new().bold());
        let table = Table::new(table_rows, widths)
            .column_spacing(1)
            .style(Style::new().blue())
            .header(header)
            .highlight_style(Style::new().reversed());

        frame.render_stateful_widget(table, rect, ui_state.table_state());
    }

    fn draw_playback_bar(
        frame: &mut Frame,
        app: &mut PlayerApp,
        _ui_state: &mut UiState,
        rect: Rect,
    ) {
        let total_duration = app
            .active_song()
            .map(|s| s.duration().as_secs_f64())
            .unwrap_or(1.0);

        let elapsed_duration = match app.active_song() {
            Some(_) => app
                .audio_manager()
                .playback_progress()
                .as_secs_f64()
                .min(total_duration),
            None => 0.0,
        };

        let playback_progress = elapsed_duration / total_duration;

        let playback_fmt = match app.active_song() {
            Some(_) => {
                let minutes = (elapsed_duration as i64) / 60;
                let secs = (elapsed_duration as i64) % 60;
                format!("{:02}:{:02}", minutes, secs)
            }
            None => String::from("--:--"),
        };

        let total_fmt = match app.active_song() {
            Some(s) => {
                let total_minutes = s.duration().as_secs() / 60;
                let total_secs = s.duration().as_secs() % 60;
                format!("{:02}:{:02}", total_minutes, total_secs)
            }
            None => String::from("--:--"),
        };

        let display_volume = (100.0 * app.volume()) as u32;
        let playback_divider = if app.is_playing() { "▶" } else { "⏸︎" };

        let playback_bar = Gauge::default()
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Playback")
                    .title(
                        Title::from(format!("Volume: {display_volume}%"))
                            .position(Position::Bottom),
                    ),
            )
            .gauge_style(
                Style::default()
                    .fg(Color::White)
                    .bg(Color::Black)
                    .add_modifier(Modifier::BOLD),
            )
            .label(format!("{playback_fmt} {playback_divider} {total_fmt}",))
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
    table_state: TableState,
}

impl UiState {
    pub fn new() -> Self {
        Self {
            table_state: TableState::default(),
        }
    }

    pub fn table_state(&mut self) -> &mut TableState {
        &mut self.table_state
    }
}
