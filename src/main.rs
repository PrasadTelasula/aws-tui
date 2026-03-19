mod app;
mod completer;
mod instances;
mod parser;
mod session;
mod terminal;
mod ui;

use app::{App, AppTab, ConfirmAction, InputMode};
use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
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
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen
    )?;
    terminal.show_cursor()?;

    if let Err(err) = result {
        eprintln!("\x1b[31m✗ Error:\x1b[0m {}", err);
        std::process::exit(1);
    }

    Ok(())
}

fn key_to_pty_bytes(key: crossterm::event::KeyEvent) -> Option<Vec<u8>> {
    use crossterm::event::{KeyCode, KeyModifiers};
    match key.code {
        KeyCode::Char(c) if key.modifiers.contains(KeyModifiers::CONTROL) => {
            match c.to_ascii_lowercase() {
                'a'..='z' => Some(vec![c.to_ascii_lowercase() as u8 - b'a' + 1]),
                '[' => Some(vec![27]),
                '\\' => Some(vec![28]),
                ']' => Some(vec![29]),
                _ => None,
            }
        }
        KeyCode::Char(c) => {
            let mut buf = [0u8; 4];
            let s = c.encode_utf8(&mut buf);
            Some(s.as_bytes().to_vec())
        }
        KeyCode::Enter => Some(vec![b'\r']),
        KeyCode::Backspace => Some(vec![127]),
        KeyCode::Delete => Some(b"\x1b[3~".to_vec()),
        KeyCode::Esc => Some(vec![27]),
        KeyCode::Tab => Some(vec![b'\t']),
        KeyCode::BackTab => Some(b"\x1b[Z".to_vec()),
        KeyCode::Up => Some(b"\x1b[A".to_vec()),
        KeyCode::Down => Some(b"\x1b[B".to_vec()),
        KeyCode::Right => Some(b"\x1b[C".to_vec()),
        KeyCode::Left => Some(b"\x1b[D".to_vec()),
        KeyCode::Home => Some(b"\x1b[H".to_vec()),
        KeyCode::End => Some(b"\x1b[F".to_vec()),
        KeyCode::PageUp => Some(b"\x1b[5~".to_vec()),
        KeyCode::PageDown => Some(b"\x1b[6~".to_vec()),
        _ => None,
    }
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

        // Keep PTY size in sync with the actual SSM panel dimensions
        if app.active_tab == AppTab::Instances {
            if let Ok(size) = terminal.size() {
                let inner_w = size.width.saturating_sub(2);
                let left_w = (inner_w * 36) / 100;
                let pty_cols = inner_w.saturating_sub(left_w + 1);
                // Session tab bar adds 1 row when sessions are open
                let tab_row: u16 = if app.instances_state.ssm_sessions.is_empty() { 0 } else { 1 };
                let pty_rows = size.height.saturating_sub(9 + tab_row);
                app.instances_state.resize_pty(pty_rows.max(4), pty_cols.max(20));
            }
        }

        if event::poll(tick_rate)? {
            if let Event::Key(key) = event::read()? {
                // ── Credentials popup ──
                if app.show_credentials_popup {
                    if key.code == KeyCode::Char('c') {
                        app.copy_credentials_to_clipboard();
                    }
                    app.show_credentials_popup = false;
                    continue;
                }

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
                    KeyCode::F(3) => {
                        app.active_tab = AppTab::Instances;
                        app.input_mode = InputMode::Normal;
                        // Auto-fetch instances if we have a profile and none loaded
                        if !app.instances_state.profiles.is_empty()
                            && app.instances_state.instances.is_empty()
                            && !app.instances_state.loading_instances
                        {
                            app.instances_state.fetch_instances();
                        }
                        continue;
                    }
                    _ => {}
                }

                // ── Terminal input mode ──
                if app.input_mode == InputMode::TerminalInput {
                    // Ctrl+H / Ctrl+L: switch terminals (H=left, L=right)
                    if key.modifiers.contains(KeyModifiers::CONTROL) {
                        match key.code {
                            KeyCode::Char('h') => { app.terminal_state.prev_terminal(); continue; }
                            KeyCode::Char('l') => { app.terminal_state.next_terminal(); continue; }
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
                        // Shift+Up/Down: scroll output (reliable in all terminals)
                        KeyCode::Up if key.modifiers.contains(KeyModifiers::SHIFT) => {
                            term.scroll_up(3);
                        }
                        KeyCode::Down if key.modifiers.contains(KeyModifiers::SHIFT) => {
                            term.scroll_down(3);
                        }
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

                // ── Instance search mode ──
                if app.instances_state.search_active
                    && app.active_tab == AppTab::Instances
                    && app.input_mode == InputMode::Normal
                {
                    match key.code {
                        KeyCode::Esc | KeyCode::Enter => {
                            app.instances_state.search_active = false;
                            if key.code == KeyCode::Esc {
                                app.instances_state.search_query.clear();
                                app.instances_state.filtered_instances.clear();
                            }
                        }
                        KeyCode::Backspace => {
                            app.instances_state.search_query.pop();
                            app.instances_state.update_instance_search();
                        }
                        KeyCode::Char(c) => {
                            app.instances_state.search_query.push(c);
                            app.instances_state.update_instance_search();
                        }
                        _ => {}
                    }
                    continue;
                }

                // ── SSM Input mode (Instances tab) — forward raw bytes to PTY ──
                if app.input_mode == InputMode::SsmInput {
                    match key.code {
                        // Ctrl+D: close active session
                        KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            app.instances_state.disconnect_ssm();
                            if app.instances_state.ssm_sessions.is_empty() {
                                app.input_mode = InputMode::Normal;
                                app.instances_state.focus = instances::InstanceFocus::InstanceList;
                            }
                        }
                        // F4 / F5: switch between open sessions (no conflict — F1-F3 are global)
                        KeyCode::F(4) => { app.instances_state.prev_session(); }
                        KeyCode::F(5) => { app.instances_state.next_session(); }
                        // Tab: exit SSM input, cycle focus
                        KeyCode::Tab => {
                            app.input_mode = InputMode::Normal;
                            app.instances_state.cycle_focus();
                        }
                        // Everything else: forward as raw bytes to PTY
                        _ => {
                            if let Some(bytes) = key_to_pty_bytes(key) {
                                app.instances_state.write_input(&bytes);
                            }
                        }
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
                    KeyCode::Char('r') if key.modifiers.contains(KeyModifiers::CONTROL)
                        && app.active_tab == AppTab::Sessions
                        && app.input_mode == InputMode::Normal =>
                    {
                        app.reload_aliases();
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
                    KeyCode::Char('h') if app.active_tab == AppTab::Terminal => {
                        app.terminal_state.prev_terminal();
                    }
                    KeyCode::Char('l') if app.active_tab == AppTab::Terminal => {
                        app.terminal_state.next_terminal();
                    }

                    // Instances tab: normal mode keys
                    KeyCode::Tab if app.active_tab == AppTab::Instances => {
                        app.instances_state.cycle_focus();
                        // Auto-enter SsmInput mode when focusing SSM terminal
                        if app.instances_state.focus == instances::InstanceFocus::SsmTerminal {
                            app.input_mode = InputMode::SsmInput;
                        }
                    }
                    KeyCode::Up | KeyCode::Char('k') if app.active_tab == AppTab::Instances => {
                        if app.instances_state.region_dropdown_open {
                            app.instances_state.prev_region();
                        } else {
                            match app.instances_state.focus {
                                instances::InstanceFocus::RegionList => {},
                                instances::InstanceFocus::InstanceList => app.instances_state.prev_instance(),
                                instances::InstanceFocus::SsmTerminal => {},
                            }
                        }
                    }
                    KeyCode::Down | KeyCode::Char('j') if app.active_tab == AppTab::Instances => {
                        if app.instances_state.region_dropdown_open {
                            app.instances_state.next_region();
                        } else {
                            match app.instances_state.focus {
                                instances::InstanceFocus::RegionList => {},
                                instances::InstanceFocus::InstanceList => app.instances_state.next_instance(),
                                instances::InstanceFocus::SsmTerminal => {},
                            }
                        }
                    }
                    KeyCode::Enter if app.active_tab == AppTab::Instances => {
                        if app.instances_state.region_dropdown_open {
                            app.instances_state.region_dropdown_open = false;
                            app.instances_state.fetch_instances();
                        } else {
                            match app.instances_state.focus {
                                instances::InstanceFocus::RegionList => {
                                    app.instances_state.region_dropdown_open = true;
                                }
                                instances::InstanceFocus::InstanceList => {
                                    let size = terminal.size().unwrap_or_default();
                                    // Approximate SSM panel: right ~64% width, minus header/footer
                                    let rows = size.height.saturating_sub(5);
                                    let cols = (size.width * 64 / 100).saturating_sub(2);
                                    app.instances_state.connect_ssm(rows.max(10), cols.max(20));
                                    app.instances_state.focus = instances::InstanceFocus::SsmTerminal;
                                    app.input_mode = InputMode::SsmInput;
                                }
                                instances::InstanceFocus::SsmTerminal => {}
                            }
                        }
                    }
                    KeyCode::Esc if app.active_tab == AppTab::Instances => {
                        if app.instances_state.region_dropdown_open {
                            app.instances_state.region_dropdown_open = false;
                        }
                    }
                    KeyCode::Char('/') if app.active_tab == AppTab::Instances
                        && app.instances_state.focus == instances::InstanceFocus::InstanceList =>
                    {
                        app.instances_state.search_active = true;
                        app.instances_state.search_query.clear();
                        app.instances_state.filtered_instances.clear();
                    }
                    KeyCode::Esc if app.active_tab == AppTab::Instances
                        && !app.instances_state.search_query.is_empty() =>
                    {
                        app.instances_state.search_query.clear();
                        app.instances_state.filtered_instances.clear();
                    }
                    KeyCode::Char('r') if app.active_tab == AppTab::Instances => {
                        app.instances_state.fetch_instances();
                    }
                    KeyCode::Char('i') if app.active_tab == AppTab::Instances => {
                        if !app.instances_state.ssm_sessions.is_empty() {
                            app.instances_state.focus = instances::InstanceFocus::SsmTerminal;
                            app.input_mode = InputMode::SsmInput;
                        }
                    }
                    // [ / ] — cycle between open SSM sessions in normal mode
                    KeyCode::Char('[') if app.active_tab == AppTab::Instances => {
                        app.instances_state.prev_session();
                    }
                    KeyCode::Char(']') if app.active_tab == AppTab::Instances => {
                        app.instances_state.next_session();
                    }
                    KeyCode::Char('h') if key.modifiers.contains(KeyModifiers::CONTROL)
                        && app.active_tab == AppTab::Instances =>
                    {
                        app.instances_state.prev_profile();
                        app.instances_state.fetch_instances();
                    }
                    KeyCode::Char('l') if key.modifiers.contains(KeyModifiers::CONTROL)
                        && app.active_tab == AppTab::Instances =>
                    {
                        app.instances_state.next_profile();
                        app.instances_state.fetch_instances();
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
                        // Interactive SSM shell: suspend TUI, hand terminal to plugin
                        if let Some(cmd) = app.pending_ssm_command.take() {
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
                        // Open credentials popup for connected SSO or IAM sessions;
                        // otherwise fall back to go-to-top.
                        let is_cred_connected = app.aliases.get(app.selected_index)
                            .map(|a| matches!(
                                a.kind,
                                crate::parser::AliasKind::SsoLogin { .. }
                                    | crate::parser::AliasKind::IamProfile { .. }
                            ))
                            .unwrap_or(false)
                            && app.session_credentials.contains_key(
                                app.aliases.get(app.selected_index)
                                    .map(|a| a.name.as_str()).unwrap_or(""));
                        if is_cred_connected {
                            app.show_credentials_popup = true;
                        } else {
                            app.selected_index = 0;
                        }
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
