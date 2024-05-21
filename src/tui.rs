use std::io::{stdout, Stdout};

use crossterm::{
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use eyre::Result;
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::Line,
    widgets::{
        block::{Position, Title},
        Block, Borders, Gauge, Paragraph, Row, Table, TableState,
    },
    Frame, Terminal,
};

use crate::app::{AppUiMode, PlaybackMode, PlayerApp};

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

    fn draw_ui_file_list_mode(frame: &mut Frame, app: &mut PlayerApp, ui_state: &mut UiState) {
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

    fn draw_ui_search_mode(frame: &mut Frame, app: &mut PlayerApp, ui_state: &mut UiState) {
        let layout =
            Layout::vertical([Constraint::Fill(1), Constraint::Fill(8), Constraint::Min(3)])
                .split(frame.size());
        let bottom_layout =
            Layout::horizontal([Constraint::Fill(4), Constraint::Min(1)]).split(layout[2]);

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

        let search_text = app.search_query().unwrap_or("Search...");
        frame.render_widget(Line::from(search_text), layout[0]);

        let tag_info =
            Paragraph::new(tags).block(Block::default().title("Now Playing").borders(Borders::ALL));
        frame.render_widget(tag_info, bottom_layout[1]);

        Self::draw_file_list(frame, app, ui_state, layout[1]);
        Self::draw_playback_bar(frame, app, ui_state, bottom_layout[0]);
    }

    fn ui(frame: &mut Frame, app: &mut PlayerApp, ui_state: &mut UiState) {
        match app.ui_mode() {
            AppUiMode::FileList => Self::draw_ui_file_list_mode(frame, app, ui_state),
            AppUiMode::SearchPopup => Self::draw_ui_search_mode(frame, app, ui_state),
            AppUiMode::InfoPopup => todo!(),
        }
    }

    fn draw_file_list(frame: &mut Frame, app: &mut PlayerApp, ui_state: &mut UiState, rect: Rect) {
        let table_rows = app
            .library()
            .files()
            .iter()
            .filter(|s| {
                // TODO: If the current selected row ix is > the length of the filtered search
                // results, the selection disappears. It doesn't crash but is annoying.
                if let Some(q) = app.search_query() {
                    let query = q.to_lowercase();
                    let title = s.title().unwrap_or("").to_lowercase();
                    let artist = s.artist().unwrap_or("").to_lowercase();
                    let album = s.album().unwrap_or("").to_lowercase();
                    title.contains(&query) || artist.contains(&query) || album.contains(&query)
                } else {
                    true
                }
            })
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
            Constraint::Fill(2), // Duration
        ];
        let header =
            Row::new(["#", "Title", "Artist", "Album", "Length"]).style(Style::new().bold());
        let table = Table::new(table_rows, widths)
            .column_spacing(1)
            .style(Style::new().bg(Color::Black).fg(Color::White))
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
            .map_or(1.0, |s| s.duration().as_secs_f64());

        let elapsed_duration = match app.active_song() {
            Some(_) => app
                .audio_manager()
                .playback_progress()
                .as_secs_f64()
                .min(total_duration),
            None => 0.0,
        };

        let playback_progress = elapsed_duration / total_duration;

        #[allow(clippy::cast_possible_truncation)]
        let playback_fmt = match app.active_song() {
            Some(_) => {
                let minutes = (elapsed_duration as i64) / 60;
                let secs = (elapsed_duration as i64) % 60;
                format!("{minutes:02}:{secs:02}")
            }
            None => String::from("--:--"),
        };

        let total_fmt = match app.active_song() {
            Some(s) => {
                let total_minutes = s.duration().as_secs() / 60;
                let total_secs = s.duration().as_secs() % 60;
                format!("{total_minutes:02}:{total_secs:02}")
            }
            None => String::from("--:--"),
        };

        #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
        let display_volume = (100.0 * app.volume()) as u32;
        let playback_divider = if app.is_playing() { "" } else { "" };
        let active_color = if app.is_playing() {
            Color::Green
        } else {
            Color::Yellow
        };

        let tags = match app.active_song() {
            Some(t) => {
                format!(
                    "{} - {}",
                    t.title().unwrap_or("Unknown Title"),
                    t.artist().unwrap_or("Unknown Artist"),
                )
            }
            _ => String::new(),
        };

        let shuffle_icon = match app.playback_mode() {
            PlaybackMode::Normal => "",
            PlaybackMode::Shuffle => "",
        }
        .to_string();

        let playback_bar = Gauge::default()
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(tags)
                    .title(
                        Title::from(format!("Volume: {display_volume}%"))
                            .position(Position::Bottom),
                    )
                    .title(
                        Title::from(shuffle_icon)
                            .position(Position::Top)
                            .alignment(Alignment::Right),
                    ),
            )
            .gauge_style(
                Style::default()
                    .fg(active_color)
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
            eprintln!("Error disabling raw mode: {e}");
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
