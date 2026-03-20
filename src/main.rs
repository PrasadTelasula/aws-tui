mod app;
mod completer;
mod containers;
mod events;
mod instances;
mod parser;
mod session;
mod terminal;
mod ui;

use app::{App, AppTab, InputMode};
use crossterm::{
    event::{self, Event},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use parser::parse_alias_file;
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use std::path::PathBuf;
use std::time::Duration;

fn resolve_alias_file() -> PathBuf {
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 {
        return PathBuf::from(&args[1]);
    }

    if let Ok(path) = std::env::var("AWS_TUI_ALIASES") {
        return PathBuf::from(path);
    }

    let home = dirs::home_dir().unwrap_or_default();
    let candidates = [
        home.join(".zsh_aliases"),
        home.join(".profile.d/aws.plugin.zsh"),
        home.join(".aws_aliases"),
        home.join(".bash_aliases"),
    ];

    for path in &candidates {
        if path.exists() {
            return path.clone();
        }
    }

    let sample = PathBuf::from("sample_aliases");
    if sample.exists() {
        return sample;
    }

    home.join(".zsh_aliases")
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let alias_file = resolve_alias_file();
    let aliases = parse_alias_file(&alias_file);

    if aliases.is_empty() {
        eprintln!(
            "\x1b[1;33m⚡ AWS Session Manager TUI\x1b[0m\n\
             \n\
             \x1b[31m✗\x1b[0m No aliases found in: \x1b[36m{}\x1b[0m\n\
             \n\
             \x1b[1mUsage:\x1b[0m  aws-tui [alias-file]\n\
             \n\
             Or set \x1b[33mAWS_TUI_ALIASES\x1b[0m environment variable.\n\
             \n\
             The alias file should contain shell aliases like:\n\
             \n\
               \x1b[32malias\x1b[0m mydb=\x1b[36m'aws ssm start-session --target i-xxx ...'\x1b[0m\n\
               \x1b[32malias\x1b[0m mylogin=\x1b[36m'aws sso login --sso-session my-session'\x1b[0m\n\
             \n\
             A sample_aliases file is included for testing:\n\
             \n\
               \x1b[1maws-tui sample_aliases\x1b[0m",
            alias_file.display()
        );
        std::process::exit(1);
    }

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(aliases, alias_file);
    app.check_existing_sessions().await;

    let result = run_app(&mut terminal, &mut app).await;

    app.stop_all_sessions().await;
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    if let Err(err) = result {
        eprintln!("\x1b[31m✗ Error:\x1b[0m {}", err);
        std::process::exit(1);
    }

    Ok(())
}

async fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
) -> Result<(), Box<dyn std::error::Error>> {
    let tick_rate = Duration::from_millis(50);

    loop {
        app.process_output_messages().await;
        app.refresh_statuses().await;
        app.on_tick();

        // Drive completer on active terminal tab
        if app.active_tab == AppTab::Terminal && app.input_mode == InputMode::TerminalInput {
            let term = app.terminal_state.active();
            if term.completer.should_query(&term.input) {
                let input = term.input.clone();
                app.terminal_state.active_mut().completer.query(&input).await;
            }
        }

        terminal.draw(|f| ui::draw(f, app))?;

        // Keep PTY size in sync with the SSM panel dimensions
        if app.active_tab == AppTab::Instances {
            if let Ok(size) = terminal.size() {
                let inner_w  = size.width.saturating_sub(2);
                let left_w   = (inner_w * 36) / 100;
                let pty_cols = inner_w.saturating_sub(left_w + 1);
                let tab_row: u16 = if app.instances_state.ssm_sessions.is_empty() { 0 } else { 1 };
                let pty_rows = size.height.saturating_sub(9 + tab_row);
                app.instances_state.resize_pty(pty_rows.max(4), pty_cols.max(20));
            }
        }

        if event::poll(tick_rate)? {
            if let Event::Key(key) = event::read()? {
                let terminal_size = terminal.size().unwrap_or_default();
                if let Some(cmd) = events::handle_key_event(app, key, terminal_size).await {
                    // Interactive SSM shell: suspend TUI, hand terminal to plugin, resume
                    disable_raw_mode()?;
                    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
                    terminal.show_cursor()?;

                    let _ = std::process::Command::new("sh")
                        .arg("-c")
                        .arg(&cmd)
                        .stdin(std::process::Stdio::inherit())
                        .stdout(std::process::Stdio::inherit())
                        .stderr(std::process::Stdio::inherit())
                        .status();

                    enable_raw_mode()?;
                    execute!(terminal.backend_mut(), EnterAlternateScreen)?;
                    terminal.clear()?;
                }
            }
        }

        if app.should_quit {
            return Ok(());
        }
    }
}
