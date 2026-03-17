use crate::parser::{Alias, AliasKind};
use crate::session::{SessionKind, SessionManager, SessionStatus};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;
use tokio::sync::mpsc;

#[derive(Debug, Clone, PartialEq)]
pub enum ActivePanel {
    AliasList,
    DetailPanel,
}

#[derive(Debug, Clone, PartialEq)]
pub enum InputMode {
    Normal,
    Search,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ConfirmAction {
    None,
    StopAll,
    Quit,
}

/// Notification toast
#[derive(Debug, Clone)]
pub struct Toast {
    pub message: String,
    pub kind: ToastKind,
    pub created_at: Instant,
}

#[derive(Debug, Clone)]
pub enum ToastKind {
    Success,
    Error,
    Info,
}

pub struct App {
    pub aliases: Vec<Alias>,
    pub selected_index: usize,
    pub active_panel: ActivePanel,
    pub input_mode: InputMode,
    pub search_query: String,
    pub filtered_indices: Vec<usize>,
    pub session_statuses: Vec<SessionStatus>,
    pub session_outputs: HashMap<String, Vec<String>>,
    pub session_pids: HashMap<usize, Option<u32>>,
    pub session_start_times: HashMap<String, Instant>,
    /// SSO token expiry: (expires_at_str, remaining_secs)
    pub token_expiry: HashMap<String, (String, u64)>,
    pub session_manager: SessionManager,
    pub output_tx: mpsc::UnboundedSender<(String, String)>,
    pub output_rx: mpsc::UnboundedReceiver<(String, String)>,
    pub running_count: usize,
    pub should_quit: bool,
    pub show_confirm: bool,
    pub confirm_message: String,
    pub confirm_action: ConfirmAction,
    pub _alias_file: PathBuf,
    pub list_scroll_offset: usize,

    // Animation state
    pub tick: u64,
    pub spinner_frame: usize,
    pub cursor_visible: bool,
    pub toast: Option<Toast>,
    pub start_time: Instant,
}

// Braille spinner frames
pub const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

pub const ICON_CHECK: &str = "✓";
pub const ICON_CROSS: &str = "✗";
pub const ICON_STOP: &str = "■";

impl App {
    pub fn new(aliases: Vec<Alias>, alias_file: PathBuf) -> Self {
        let statuses = vec![SessionStatus::Stopped; aliases.len()];
        let (tx, rx) = mpsc::unbounded_channel();

        Self {
            aliases,
            selected_index: 0,
            active_panel: ActivePanel::AliasList,
            input_mode: InputMode::Normal,
            search_query: String::new(),
            filtered_indices: Vec::new(),
            session_statuses: statuses,
            session_outputs: HashMap::new(),
            session_pids: HashMap::new(),
            session_start_times: HashMap::new(),
            token_expiry: HashMap::new(),
            session_manager: SessionManager::new(),
            output_tx: tx,
            output_rx: rx,
            running_count: 0,
            should_quit: false,
            show_confirm: false,
            confirm_message: String::new(),
            confirm_action: ConfirmAction::None,
            _alias_file: alias_file,
            list_scroll_offset: 0,
            tick: 0,
            spinner_frame: 0,
            cursor_visible: true,
            toast: None,
            start_time: Instant::now(),
        }
    }

    pub fn on_tick(&mut self) {
        self.tick += 1;
        self.spinner_frame = (self.tick as usize / 2) % SPINNER_FRAMES.len();
        self.cursor_visible = (self.tick / 5) % 2 == 0;

        // Clear expired toasts (after 3 seconds)
        if let Some(ref toast) = self.toast {
            if toast.created_at.elapsed().as_secs() >= 3 {
                self.toast = None;
            }
        }
    }

    pub fn show_toast(&mut self, message: String, kind: ToastKind) {
        self.toast = Some(Toast {
            message,
            kind,
            created_at: Instant::now(),
        });
    }

    pub fn spinner(&self) -> &str {
        SPINNER_FRAMES[self.spinner_frame]
    }

    pub fn uptime_str(&self) -> String {
        let elapsed = self.start_time.elapsed().as_secs();
        let hours = elapsed / 3600;
        let mins = (elapsed % 3600) / 60;
        let secs = elapsed % 60;
        if hours > 0 {
            format!("{:02}:{:02}:{:02}", hours, mins, secs)
        } else {
            format!("{:02}:{:02}", mins, secs)
        }
    }

    /// Returns formatted token remaining time for SSO sessions
    pub fn token_remaining_str(&self, alias_name: &str) -> Option<String> {
        self.token_expiry.get(alias_name).map(|(_, secs)| {
            let hours = secs / 3600;
            let mins = (secs % 3600) / 60;
            if hours > 0 {
                format!("{}h {:02}m", hours, mins)
            } else if mins > 0 {
                format!("{}m", mins)
            } else {
                format!("{}s", secs)
            }
        })
    }

    pub fn session_uptime(&self, alias_name: &str) -> Option<String> {
        self.session_start_times.get(alias_name).map(|start| {
            let elapsed = start.elapsed().as_secs();
            let hours = elapsed / 3600;
            let mins = (elapsed % 3600) / 60;
            let secs = elapsed % 60;
            if hours > 0 {
                format!("{}h {}m {}s", hours, mins, secs)
            } else if mins > 0 {
                format!("{}m {}s", mins, secs)
            } else {
                format!("{}s", secs)
            }
        })
    }

    pub fn next(&mut self) {
        if self.aliases.is_empty() {
            return;
        }
        if !self.search_query.is_empty() && !self.filtered_indices.is_empty() {
            let current_pos = self
                .filtered_indices
                .iter()
                .position(|&i| i == self.selected_index)
                .unwrap_or(0);
            let next_pos = (current_pos + 1) % self.filtered_indices.len();
            self.selected_index = self.filtered_indices[next_pos];
        } else {
            self.selected_index = (self.selected_index + 1) % self.aliases.len();
        }
    }

    pub fn previous(&mut self) {
        if self.aliases.is_empty() {
            return;
        }
        if !self.search_query.is_empty() && !self.filtered_indices.is_empty() {
            let current_pos = self
                .filtered_indices
                .iter()
                .position(|&i| i == self.selected_index)
                .unwrap_or(0);
            let next_pos = if current_pos == 0 {
                self.filtered_indices.len() - 1
            } else {
                current_pos - 1
            };
            self.selected_index = self.filtered_indices[next_pos];
        } else if self.selected_index == 0 {
            self.selected_index = self.aliases.len() - 1;
        } else {
            self.selected_index -= 1;
        }
    }

    pub fn toggle_panel(&mut self) {
        self.active_panel = match self.active_panel {
            ActivePanel::AliasList => ActivePanel::DetailPanel,
            ActivePanel::DetailPanel => ActivePanel::AliasList,
        };
    }

    pub fn update_search(&mut self) {
        if self.search_query.is_empty() {
            self.filtered_indices.clear();
            return;
        }
        let query = self.search_query.to_lowercase();
        self.filtered_indices = self
            .aliases
            .iter()
            .enumerate()
            .filter(|(_, alias)| {
                alias.name.to_lowercase().contains(&query)
                    || alias.group.to_lowercase().contains(&query)
                    || match &alias.kind {
                        AliasKind::SsmSession {
                            host, local_port, ..
                        } => {
                            host.as_deref()
                                .unwrap_or("")
                                .to_lowercase()
                                .contains(&query)
                                || local_port.as_deref().unwrap_or("").contains(&query)
                        }
                        AliasKind::SsoLogin { session_name } => {
                            session_name.to_lowercase().contains(&query)
                        }
                        _ => false,
                    }
            })
            .map(|(i, _)| i)
            .collect();

        if !self.filtered_indices.is_empty()
            && !self.filtered_indices.contains(&self.selected_index)
        {
            self.selected_index = self.filtered_indices[0];
        }
    }

    pub async fn start_selected(&mut self) {
        if self.aliases.is_empty() {
            return;
        }
        let alias = &self.aliases[self.selected_index];
        let name = alias.name.clone();
        let command = alias.command.clone();

        let kind = match &alias.kind {
            AliasKind::SsoLogin { session_name } => SessionKind::SsoLogin {
                session_name: session_name.clone(),
            },
            AliasKind::SsmSession { .. } => SessionKind::SsmSession,
            AliasKind::Other => SessionKind::Other,
        };

        match self
            .session_manager
            .start_session(&name, &command, kind, self.output_tx.clone())
            .await
        {
            Ok(()) => {
                self.session_statuses[self.selected_index] = SessionStatus::Running;
                let pid = self.session_manager.get_pid(&name).await;
                self.session_pids.insert(self.selected_index, pid);
                self.session_start_times.insert(name.clone(), Instant::now());
                self.show_toast(
                    format!("{} {} started", ICON_CHECK, name),
                    ToastKind::Success,
                );
            }
            Err(e) => {
                self.session_outputs
                    .entry(name.clone())
                    .or_default()
                    .push(format!(">>> Error: {}", e));
                self.show_toast(format!("{} {}", ICON_CROSS, e), ToastKind::Error);
            }
        }
    }

    pub async fn stop_selected(&mut self) {
        if self.aliases.is_empty() {
            return;
        }
        let name = self.aliases[self.selected_index].name.clone();
        match self.session_manager.stop_session(&name).await {
            Ok(()) => {
                self.session_statuses[self.selected_index] = SessionStatus::Stopped;
                self.session_pids.insert(self.selected_index, None);
                self.session_start_times.remove(&name);
                self.show_toast(
                    format!("{} {} stopped", ICON_STOP, name),
                    ToastKind::Info,
                );
            }
            Err(e) => {
                self.session_outputs
                    .entry(name)
                    .or_default()
                    .push(format!(">>> Error: {}", e));
                self.show_toast(format!("{} {}", ICON_CROSS, e), ToastKind::Error);
            }
        }
    }

    pub async fn stop_all_sessions(&mut self) {
        self.session_manager.stop_all().await;
        for i in 0..self.session_statuses.len() {
            self.session_statuses[i] = SessionStatus::Stopped;
            self.session_pids.insert(i, None);
        }
        self.session_start_times.clear();
        self.show_toast(
            format!("{} All sessions stopped", ICON_STOP),
            ToastKind::Info,
        );
    }

    pub async fn refresh_statuses(&mut self) {
        let mut count = 0;
        for (i, alias) in self.aliases.iter().enumerate() {
            let status = self.session_manager.get_status(&alias.name).await;
            match &status {
                SessionStatus::Running | SessionStatus::Starting | SessionStatus::Connected => {
                    count += 1;
                }
                SessionStatus::Stopped | SessionStatus::Expired | SessionStatus::Error(_) => {
                    self.session_start_times.remove(&alias.name);
                }
            }
            self.session_statuses[i] = status;

            let pid = self.session_manager.get_pid(&alias.name).await;
            self.session_pids.insert(i, pid);

            // Fetch token expiry for SSO sessions
            let (exp_str, exp_secs) = self.session_manager.get_token_expiry(&alias.name).await;
            if let (Some(s), Some(r)) = (exp_str, exp_secs) {
                self.token_expiry.insert(alias.name.clone(), (s, r));
            } else {
                self.token_expiry.remove(&alias.name);
            }

            let output = self.session_manager.get_output(&alias.name).await;
            if !output.is_empty() {
                self.session_outputs.insert(alias.name.clone(), output);
            }
        }
        self.running_count = count;
    }

    pub async fn process_output_messages(&mut self) {
        while let Ok((name, line)) = self.output_rx.try_recv() {
            self.session_manager
                .append_output(&name, line.clone())
                .await;
            self.session_outputs
                .entry(name)
                .or_default()
                .push(line);
        }
    }
}
