mod app;
mod config;
mod flow;
mod theme;
mod ui;

use anyhow::Result;
use app::{App, Mode};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::io;
use std::path::PathBuf;
use std::time::Duration;
use theme::Theme;

enum ArgMode {
    Tui,
    List,
}

fn parse_args() -> std::result::Result<ArgMode, String> {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        None => Ok(ArgMode::Tui),
        Some("--list") => Ok(ArgMode::List),
        Some("-h") | Some("--help") => {
            print_usage();
            std::process::exit(0);
        }
        Some(other) => Err(format!(
            "wf-echo: unrecognized argument '{other}'\nRun `wf-echo --help` for usage."
        )),
    }
}

fn print_usage() {
    println!(
        "wf-echo — traffic inspector TUI, browses WraithFlow's captured traffic\n\
         \n\
         USAGE:\n\
         \x20   wf-echo            Open the TUI flow viewer\n\
         \x20   wf-echo --list     Print every captured flow as JSON and exit\n\
         \x20   -h, --help         Print this help and exit\n\
         \n\
         Reads the capture_log path from WraithFlow's own config\n\
         (~/.config/wraithflow/config.toml) — set `capture_log = \"...\"`\n\
         there first if this comes back empty."
    );
}

/// Finds WraithFlow's configured `capture_log` path, or a clear error
/// explaining exactly what to do (not just "not found") — this is the
/// single most likely reason Echo has nothing to show on a fresh setup.
fn resolve_capture_log_path() -> Result<PathBuf> {
    let wraithflow_config = config::default_wraithflow_config_path().ok_or_else(|| {
        anyhow::anyhow!(
            "no WraithFlow config found (checked $XDG_CONFIG_HOME/wraithflow, ~/.config/wraithflow, ./config.toml)"
        )
    })?;
    config::find_capture_log_path(&wraithflow_config).ok_or_else(|| {
        anyhow::anyhow!(
            "WraithFlow config at {} has no `capture_log` set — add e.g.\n  capture_log = \"~/.local/state/wraithflow/captures.jsonl\"\nand restart wraithflow before running echo",
            wraithflow_config.display()
        )
    })
}

fn run_list() -> Result<()> {
    let log_path = resolve_capture_log_path()?;
    let flows = flow::read_all(&log_path)?;
    println!("{}", serde_json::to_string(&flows)?);
    Ok(())
}

fn run_tui() -> Result<()> {
    let log_path = resolve_capture_log_path()?;
    let theme = Theme::from_cybercore();

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(log_path)?;
    let result = event_loop(&mut terminal, &mut app, &theme);

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    result
}

fn main() -> Result<()> {
    match parse_args() {
        Ok(ArgMode::List) => run_list(),
        Ok(ArgMode::Tui) => run_tui(),
        Err(msg) => {
            eprintln!("{msg}");
            std::process::exit(2);
        }
    }
}

fn log_mtime(path: &std::path::Path) -> Option<std::time::SystemTime> {
    std::fs::metadata(path).ok()?.modified().ok()
}

/// Fully synchronous, same reasoning as WraithFlow's own `wf-tui`: an
/// async runtime or a separate watcher thread for "has this file changed"
/// would add real complexity for no real benefit over a short poll
/// timeout on the same blocking crossterm read this loop already needs.
fn event_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
    theme: &Theme,
) -> Result<()> {
    let mut last_mtime = log_mtime_for(app);

    loop {
        terminal.draw(|frame| ui::draw(frame, app, theme))?;

        if app.should_quit {
            return Ok(());
        }

        if event::poll(Duration::from_millis(300))? {
            if let Event::Key(key) = event::read()? {
                match app.mode {
                    Mode::Normal => handle_normal(app, key.code, key.modifiers),
                    Mode::Filter => handle_filter(app, key.code),
                    Mode::Detail => handle_detail(app, key.code),
                    Mode::Help => handle_help(app, key.code),
                }
            }
        } else {
            let current = log_mtime_for(app);
            if current != last_mtime {
                app.reload();
                last_mtime = current;
            }
        }

        if app.should_quit {
            return Ok(());
        }
    }
}

fn log_mtime_for(app: &App) -> Option<std::time::SystemTime> {
    log_mtime(app.log_path_for_polling())
}

fn handle_normal(app: &mut App, code: KeyCode, mods: KeyModifiers) {
    match code {
        KeyCode::Char('q') | KeyCode::Esc => app.should_quit = true,
        KeyCode::Char('j') | KeyCode::Down => app.next(),
        KeyCode::Char('k') | KeyCode::Up => app.previous(),
        KeyCode::Char('/') => app.mode = Mode::Filter,
        KeyCode::Char('f') => app.toggle_follow(),
        KeyCode::Char('R') => {
            app.reload();
            app.status = Some("reloaded.".to_string());
        }
        KeyCode::Enter | KeyCode::Char('l') => {
            if app.selected_flow().is_some() {
                app.mode = Mode::Detail;
            }
        }
        KeyCode::Char('?') => app.mode = Mode::Help,
        KeyCode::Char('c') if mods.contains(KeyModifiers::CONTROL) => app.should_quit = true,
        _ => {}
    }
}

fn handle_filter(app: &mut App, code: KeyCode) {
    match code {
        KeyCode::Esc => {
            app.filter_text.clear();
            app.apply_sort_and_filter();
            app.mode = Mode::Normal;
        }
        KeyCode::Enter => app.mode = Mode::Normal,
        KeyCode::Backspace => {
            app.filter_text.pop();
            app.apply_sort_and_filter();
        }
        KeyCode::Char(c) => {
            app.filter_text.push(c);
            app.apply_sort_and_filter();
        }
        _ => {}
    }
}

fn handle_detail(app: &mut App, code: KeyCode) {
    if matches!(code, KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('l')) {
        app.mode = Mode::Normal;
    }
}

fn handle_help(app: &mut App, code: KeyCode) {
    if matches!(code, KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('?')) {
        app.mode = Mode::Normal;
    }
}
