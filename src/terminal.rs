use crate::completer::Completer;
use std::collections::HashSet;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;

#[derive(Debug, Clone)]
pub struct CommandEntry {
    pub command: String,
    pub output_lines: Vec<String>,
    pub exit_code: Option<i32>,
    pub is_running: bool,
}

/// A live profile available for terminal use
#[derive(Debug, Clone)]
pub struct LiveProfile {
    pub profile_name: String,
    pub _alias_name: String,
    pub _session_name: String,
}

/// A single terminal instance — independent input, history, output, completer
pub struct TerminalInstance {
    pub profile: Option<String>, // None = default (no profile)
    pub input: String,
    pub cursor_pos: usize,
    pub history: Vec<String>,
    history_index: Option<usize>,
    saved_input: String,
    pub entries: Vec<CommandEntry>,
    pub scroll_offset: usize,
    pub completer: Completer,
}

impl TerminalInstance {
    pub fn new(profile: Option<String>) -> Self {
        Self {
            profile,
            input: String::new(),
            cursor_pos: 0,
            history: Vec::new(),
            history_index: None,
            saved_input: String::new(),
            entries: Vec::new(),
            scroll_offset: 0,
            completer: Completer::new(),
        }
    }

    pub fn profile_label(&self) -> &str {
        self.profile.as_deref().unwrap_or("default")
    }

    pub fn insert_char(&mut self, c: char) {
        self.input.insert(self.cursor_pos, c);
        self.cursor_pos += c.len_utf8();
        self.completer.notify_keystroke();
        self.history_index = None;
    }

    pub fn backspace(&mut self) {
        if self.cursor_pos > 0 {
            let prev = self.input[..self.cursor_pos]
                .char_indices()
                .last()
                .map(|(i, _)| i)
                .unwrap_or(0);
            self.input.remove(prev);
            self.cursor_pos = prev;
            self.completer.notify_keystroke();
        }
    }

    pub fn delete(&mut self) {
        if self.cursor_pos < self.input.len() {
            self.input.remove(self.cursor_pos);
            self.completer.notify_keystroke();
        }
    }

    pub fn cursor_left(&mut self) {
        if self.cursor_pos > 0 {
            self.cursor_pos = self.input[..self.cursor_pos]
                .char_indices()
                .last()
                .map(|(i, _)| i)
                .unwrap_or(0);
        }
    }

    pub fn cursor_right(&mut self) {
        if self.cursor_pos < self.input.len() {
            if let Some(c) = self.input[self.cursor_pos..].chars().next() {
                self.cursor_pos += c.len_utf8();
            }
        }
    }

    pub fn cursor_home(&mut self) { self.cursor_pos = 0; }
    pub fn cursor_end(&mut self) { self.cursor_pos = self.input.len(); }

    pub fn clear_line(&mut self) {
        self.input.clear();
        self.cursor_pos = 0;
        self.completer.dismiss();
    }

    pub fn delete_word_backward(&mut self) {
        if self.cursor_pos == 0 { return; }
        let before = &self.input[..self.cursor_pos];
        let trimmed = before.trim_end();
        let word_start = trimmed.rfind(' ').map(|i| i + 1).unwrap_or(0);
        self.input.drain(word_start..self.cursor_pos);
        self.cursor_pos = word_start;
        self.completer.notify_keystroke();
    }

    pub fn history_up(&mut self) {
        if self.history.is_empty() { return; }
        match self.history_index {
            None => {
                self.saved_input = self.input.clone();
                self.history_index = Some(self.history.len() - 1);
                self.input = self.history[self.history.len() - 1].clone();
            }
            Some(0) => {}
            Some(idx) => {
                self.history_index = Some(idx - 1);
                self.input = self.history[idx - 1].clone();
            }
        }
        self.cursor_pos = self.input.len();
        self.completer.dismiss();
    }

    pub fn history_down(&mut self) {
        match self.history_index {
            None => {}
            Some(idx) => {
                if idx + 1 >= self.history.len() {
                    self.history_index = None;
                    self.input = self.saved_input.clone();
                } else {
                    self.history_index = Some(idx + 1);
                    self.input = self.history[idx + 1].clone();
                }
            }
        }
        self.cursor_pos = self.input.len();
        self.completer.dismiss();
    }

    pub fn total_output_lines(&self) -> usize {
        self.entries.iter().map(|e| 1 + e.output_lines.len() + 1).sum()
    }

    pub fn scroll_up(&mut self, amount: usize) {
        let max = self.total_output_lines().saturating_sub(1);
        self.scroll_offset = (self.scroll_offset + amount).min(max);
    }

    pub fn scroll_down(&mut self, amount: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(amount);
    }
}

/// Top-level terminal state — manages multiple terminal instances
pub struct TerminalState {
    pub terminals: Vec<TerminalInstance>,
    pub active_idx: usize,

    /// Available live profiles (refreshed from app state)
    pub live_profiles: Vec<LiveProfile>,

    /// Channel for receiving command output
    cmd_tx: mpsc::UnboundedSender<(usize, String, bool)>,
    cmd_rx: mpsc::UnboundedReceiver<(usize, String, bool)>,

    /// Global entry counter (across all terminals) for unique IDs
    next_entry_id: usize,
    /// Maps entry_id → (terminal_index, entry_index_within_terminal)
    entry_map: std::collections::HashMap<usize, (usize, usize)>,
}

impl TerminalState {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        Self {
            terminals: vec![TerminalInstance::new(None)], // "default" terminal
            active_idx: 0,
            live_profiles: Vec::new(),
            cmd_tx: tx,
            cmd_rx: rx,
            next_entry_id: 0,
            entry_map: std::collections::HashMap::new(),
        }
    }

    /// Get the active terminal instance
    pub fn active(&self) -> &TerminalInstance {
        &self.terminals[self.active_idx]
    }

    /// Get the active terminal instance mutably
    pub fn active_mut(&mut self) -> &mut TerminalInstance {
        &mut self.terminals[self.active_idx]
    }

    /// Switch to next terminal
    pub fn next_terminal(&mut self) {
        if !self.terminals.is_empty() {
            self.active_idx = (self.active_idx + 1) % self.terminals.len();
        }
    }

    /// Switch to previous terminal
    pub fn prev_terminal(&mut self) {
        if !self.terminals.is_empty() {
            self.active_idx = if self.active_idx == 0 {
                self.terminals.len() - 1
            } else {
                self.active_idx - 1
            };
        }
    }

    /// Sync terminals with live profiles. Creates new terminals for new profiles,
    /// keeps existing ones (to preserve history).
    pub fn sync_profiles(&mut self) {
        let existing: HashSet<Option<String>> = self.terminals
            .iter()
            .map(|t| t.profile.clone())
            .collect();

        for profile in &self.live_profiles {
            let key = Some(profile.profile_name.clone());
            if !existing.contains(&key) {
                self.terminals.push(TerminalInstance::new(key));
            }
        }

        // Ensure active_idx is valid
        if self.active_idx >= self.terminals.len() {
            self.active_idx = 0;
        }
    }

    /// Process incoming command output from all running subprocesses
    pub fn tick(&mut self) {
        while let Ok((entry_id, line, is_stderr)) = self.cmd_rx.try_recv() {
            if let Some(&(term_idx, entry_idx)) = self.entry_map.get(&entry_id) {
                if let Some(term) = self.terminals.get_mut(term_idx) {
                    if line.starts_with("__EXIT__:") {
                        if let Some(entry) = term.entries.get_mut(entry_idx) {
                            let code_str = &line["__EXIT__:".len()..];
                            entry.exit_code = code_str.parse().ok();
                            entry.is_running = false;
                        }
                        continue;
                    }

                    if let Some(entry) = term.entries.get_mut(entry_idx) {
                        let formatted = if is_stderr {
                            format!("[stderr] {}", line)
                        } else {
                            line
                        };
                        entry.output_lines.push(formatted);
                        if entry.output_lines.len() > 1000 {
                            entry.output_lines.drain(..entry.output_lines.len() - 1000);
                        }
                    }
                }
            }
        }
    }

    /// Execute command in the active terminal
    pub async fn execute(&mut self) {
        let term_idx = self.active_idx;
        let term = &mut self.terminals[term_idx];

        let cmd = term.input.trim().to_string();
        if cmd.is_empty() { return; }

        // History
        if term.history.last().map(|h| h.as_str()) != Some(&cmd) {
            term.history.push(cmd.clone());
        }
        if term.history.len() > 500 {
            term.history.drain(..term.history.len() - 500);
        }

        term.history_index = None;
        term.input.clear();
        term.cursor_pos = 0;
        term.completer.dismiss();
        term.scroll_offset = 0;

        if cmd == "clear" {
            term.entries.clear();
            return;
        }

        let profile = term.profile.clone();
        let entry_idx = term.entries.len();
        term.entries.push(CommandEntry {
            command: cmd.clone(),
            output_lines: Vec::new(),
            exit_code: None,
            is_running: true,
        });

        // Register in global entry map
        let entry_id = self.next_entry_id;
        self.next_entry_id += 1;
        self.entry_map.insert(entry_id, (term_idx, entry_idx));

        let tx = self.cmd_tx.clone();
        tokio::spawn(async move {
            let mut command = Command::new("sh");
            command
                .arg("-c")
                .arg(&cmd)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .stdin(Stdio::null());

            if let Some(ref prof) = profile {
                command.env("AWS_PROFILE", prof);
            }

            let result = command.spawn();

            match result {
                Ok(mut child) => {
                    let stdout = child.stdout.take();
                    let stderr = child.stderr.take();

                    let tx_out = tx.clone();
                    let stdout_handle = stdout.map(|out| {
                        tokio::spawn(async move {
                            let reader = BufReader::new(out);
                            let mut lines = reader.lines();
                            while let Ok(Some(line)) = lines.next_line().await {
                                let _ = tx_out.send((entry_id, line, false));
                            }
                        })
                    });

                    let tx_err = tx.clone();
                    let stderr_handle = stderr.map(|err| {
                        tokio::spawn(async move {
                            let reader = BufReader::new(err);
                            let mut lines = reader.lines();
                            while let Ok(Some(line)) = lines.next_line().await {
                                let _ = tx_err.send((entry_id, line, true));
                            }
                        })
                    });

                    let status = child.wait().await;
                    if let Some(h) = stdout_handle { let _ = h.await; }
                    if let Some(h) = stderr_handle { let _ = h.await; }

                    let exit_code = status.ok().and_then(|s| s.code()).unwrap_or(-1);
                    let _ = tx.send((entry_id, format!("__EXIT__:{}", exit_code), false));
                }
                Err(e) => {
                    let _ = tx.send((entry_id, format!("Failed to spawn: {}", e), true));
                    let _ = tx.send((entry_id, "__EXIT__:-1".to_string(), false));
                }
            }
        });
    }
}
