mod app;
mod completer;
mod parser;
mod session;
mod terminal;
mod ui;

use app::{App, AppTab, ConfirmAction, InputMode};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
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

    let mut sorted_aliases = aliases;
    sorted_aliases.sort_by(|a, b| {
        let kind_order = |k: &parser::AliasKind| -> u8 {
            match k {
                parser::AliasKind::SsoLogin { .. } => 0,
                parser::AliasKind::SsmSession { .. } => 1,
                parser::AliasKind::Other => 2,
            }
        };
        a.group
            .cmp(&b.group)
            .then(kind_order(&a.kind).cmp(&kind_order(&b.kind)))
            .then(a.name.cmp(&b.name))
    });

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(sorted_aliases, alias_file);
    app.check_existing_sessions().await;

    let result = run_app(&mut terminal, &mut app).await;

    app.stop_all_sessions().await;
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
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

        // Drive completer (debounced) on active terminal
        if app.active_tab == AppTab::Terminal && app.input_mode == InputMode::TerminalInput {
            let term = app.terminal_state.active();
            if term.completer.should_query(&term.input) {
                let input = term.input.clone();
                app.terminal_state.active_mut().completer.query(&input).await;
            }
        }

        terminal.draw(|f| ui::draw(f, app))?;

        if event::poll(tick_rate)? {
            if let Event::Key(key) = event::read()? {
                // ── Confirmation popup ──
                if app.show_confirm {
                    match key.code {
                        KeyCode::Char('y') | KeyCode::Char('Y') => {
                            let action = app.confirm_action.clone();
                            app.show_confirm = false;
                            app.confirm_action = ConfirmAction::None;
                            match action {
                                ConfirmAction::StopAll => app.stop_all_sessions().await,
                                ConfirmAction::Quit => app.should_quit = true,
                                ConfirmAction::None => {}
                            }
                        }
                        _ => {
                            app.show_confirm = false;
                            app.confirm_action = ConfirmAction::None;
                        }
                    }
                    continue;
                }

                // ── Search mode (Sessions tab) ──
                if app.input_mode == InputMode::Search {
                    match key.code {
                        KeyCode::Esc => {
                            app.input_mode = InputMode::Normal;
                            app.search_query.clear();
                            app.filtered_indices.clear();
                        }
                        KeyCode::Enter => {
                            app.input_mode = InputMode::Normal;
                        }
                        KeyCode::Backspace => {
                            app.search_query.pop();
                            app.update_search();
                        }
                        KeyCode::Char(c) => {
                            app.search_query.push(c);
                            app.update_search();
                        }
                        _ => {}
                    }
                    continue;
                }

                // ── Global: F1/F2 tab switching (works in ANY mode) ──
                match key.code {
                    KeyCode::F(1) => {
                        app.active_tab = AppTab::Sessions;
                        app.input_mode = InputMode::Normal;
                        continue;
                    }
                    KeyCode::F(2) => {
                        app.active_tab = AppTab::Terminal;
                        app.input_mode = InputMode::TerminalInput;
                        continue;
                    }
                    _ => {}
                }

                // ── Terminal input mode ──
                if app.input_mode == InputMode::TerminalInput {
                    // Handle terminal switching first (needs &mut terminal_state, not &mut term)
                    if key.modifiers.contains(KeyModifiers::CONTROL) {
                        match key.code {
                            KeyCode::Left => { app.terminal_state.prev_terminal(); continue; }
                            KeyCode::Right => { app.terminal_state.next_terminal(); continue; }
                            _ => {}
                        }
                    }

                    // Handle Enter (needs &mut terminal_state for execute)
                    if key.code == KeyCode::Enter {
                        app.terminal_state.execute().await;
                        continue;
                    }

                    // Handle Esc
                    if key.code == KeyCode::Esc {
                        app.terminal_state.active_mut().completer.dismiss();
                        app.input_mode = InputMode::Normal;
                        continue;
                    }

                    let term = app.terminal_state.active_mut();

                    // Suggestion popup navigation
                    if term.completer.visible {
                        match key.code {
                            KeyCode::Down => { term.completer.next(); continue; }
                            KeyCode::Up => { term.completer.prev(); continue; }
                            KeyCode::Tab => {
                                if let Some(new_input) = term.completer.accept_selected(&term.input) {
                                    term.input = new_input;
                                    term.cursor_pos = term.input.len();
                                    term.completer.notify_keystroke();
                                }
                                continue;
                            }
                            _ => { term.completer.dismiss(); }
                        }
                    }

                    match key.code {
                        KeyCode::Backspace => term.backspace(),
                        KeyCode::Delete => term.delete(),
                        KeyCode::Left => term.cursor_left(),
                        KeyCode::Right => term.cursor_right(),
                        KeyCode::Up => term.history_up(),
                        KeyCode::Down => term.history_down(),
                        KeyCode::PageUp => term.scroll_up(10),
                        KeyCode::PageDown => term.scroll_down(10),
                        KeyCode::Tab => {
                            if let Some(new_input) = term.completer.accept_selected(&term.input) {
                                term.input = new_input;
                                term.cursor_pos = term.input.len();
                                term.completer.notify_keystroke();
                            }
                        }
                        KeyCode::Char('a') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            term.cursor_home();
                        }
                        KeyCode::Char('e') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            term.cursor_end();
                        }
                        KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            term.clear_line();
                        }
                        KeyCode::Char('w') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            term.delete_word_backward();
                        }
                        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            term.input.clear();
                            term.cursor_pos = 0;
                            term.completer.dismiss();
                        }
                        KeyCode::Char(c) => term.insert_char(c),
                        _ => {}
                    }
                    continue;
                }

                // ── Normal mode ──
                match key.code {
                    KeyCode::Char('q') => {
                        if app.running_count > 0 {
                            app.show_confirm = true;
                            app.confirm_message = format!(
                                "Stop {} running session(s) and quit?",
                                app.running_count
                            );
                            app.confirm_action = ConfirmAction::Quit;
                        } else {
                            app.should_quit = true;
                        }
                    }
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        app.should_quit = true;
                    }

                    // Terminal tab: normal mode keys
                    KeyCode::Char('i') if app.active_tab == AppTab::Terminal => {
                        app.input_mode = InputMode::TerminalInput;
                    }
                    KeyCode::Up | KeyCode::Char('k') if app.active_tab == AppTab::Terminal => {
                        app.terminal_state.active_mut().scroll_up(3);
                    }
                    KeyCode::Down | KeyCode::Char('j') if app.active_tab == AppTab::Terminal => {
                        app.terminal_state.active_mut().scroll_down(3);
                    }
                    KeyCode::Left if app.active_tab == AppTab::Terminal => {
                        app.terminal_state.prev_terminal();
                    }
                    KeyCode::Right if app.active_tab == AppTab::Terminal => {
                        app.terminal_state.next_terminal();
                    }

                    // Sessions tab: normal mode keys
                    KeyCode::Down | KeyCode::Char('j') if app.active_tab == AppTab::Sessions => {
                        app.next();
                    }
                    KeyCode::Up | KeyCode::Char('k') if app.active_tab == AppTab::Sessions => {
                        app.previous();
                    }
                    KeyCode::Tab | KeyCode::BackTab if app.active_tab == AppTab::Sessions => {
                        app.toggle_panel();
                    }
                    KeyCode::Enter if app.active_tab == AppTab::Sessions => {
                        app.start_selected().await;
                    }
                    KeyCode::Char('s') if app.active_tab == AppTab::Sessions => {
                        app.stop_selected().await;
                    }
                    KeyCode::Char('S') if app.active_tab == AppTab::Sessions => {
                        if app.running_count > 0 {
                            app.show_confirm = true;
                            app.confirm_message = format!(
                                "Stop all {} running session(s)?",
                                app.running_count
                            );
                            app.confirm_action = ConfirmAction::StopAll;
                        }
                    }
                    KeyCode::Char('/') if app.active_tab == AppTab::Sessions => {
                        app.input_mode = InputMode::Search;
                        app.search_query.clear();
                    }
                    KeyCode::Esc => {
                        if !app.search_query.is_empty() {
                            app.search_query.clear();
                            app.filtered_indices.clear();
                        }
                    }
                    KeyCode::Char('g') if app.active_tab == AppTab::Sessions => {
                        app.selected_index = 0;
                    }
                    KeyCode::Char('G') if app.active_tab == AppTab::Sessions => {
                        if !app.aliases.is_empty() {
                            app.selected_index = app.aliases.len() - 1;
                        }
                    }
                    _ => {}
                }
            }
        }

        if app.should_quit {
            return Ok(());
        }
    }
}
