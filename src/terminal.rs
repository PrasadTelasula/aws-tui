use crate::completer::Completer;
use std::process::Stdio;
use std::time::Instant;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;

#[derive(Debug, Clone)]
pub struct CommandEntry {
    pub command: String,
    pub output_lines: Vec<String>,
    pub exit_code: Option<i32>,
    pub is_running: bool,
    pub _started_at: Instant,
}

pub struct TerminalState {
    pub input: String,
    pub cursor_pos: usize,
    pub history: Vec<String>,
    history_index: Option<usize>,
    saved_input: String,
    pub entries: Vec<CommandEntry>,
    pub scroll_offset: usize,
    pub completer: Completer,
    cmd_tx: mpsc::UnboundedSender<(usize, String, bool)>,
    cmd_rx: mpsc::UnboundedReceiver<(usize, String, bool)>,
}

impl TerminalState {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        Self {
            input: String::new(),
            cursor_pos: 0,
            history: Vec::new(),
            history_index: None,
            saved_input: String::new(),
            entries: Vec::new(),
            scroll_offset: 0,
            completer: Completer::new(),
            cmd_tx: tx,
            cmd_rx: rx,
        }
    }

    /// Process incoming command output from running subprocesses.
    pub fn tick(&mut self) {
        while let Ok((entry_idx, line, is_stderr)) = self.cmd_rx.try_recv() {
            // Sentinel for process exit
            if line.starts_with("__EXIT__:") {
                if let Some(entry) = self.entries.get_mut(entry_idx) {
                    let code_str = &line["__EXIT__:".len()..];
                    entry.exit_code = code_str.parse().ok();
                    entry.is_running = false;
                }
                continue;
            }

            if let Some(entry) = self.entries.get_mut(entry_idx) {
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

    pub fn cursor_home(&mut self) {
        self.cursor_pos = 0;
    }

    pub fn cursor_end(&mut self) {
        self.cursor_pos = self.input.len();
    }

    pub fn clear_line(&mut self) {
        self.input.clear();
        self.cursor_pos = 0;
        self.completer.dismiss();
    }

    pub fn delete_word_backward(&mut self) {
        if self.cursor_pos == 0 {
            return;
        }
        let before = &self.input[..self.cursor_pos];
        let trimmed = before.trim_end();
        let word_start = trimmed.rfind(' ').map(|i| i + 1).unwrap_or(0);
        self.input.drain(word_start..self.cursor_pos);
        self.cursor_pos = word_start;
        self.completer.notify_keystroke();
    }

    pub fn history_up(&mut self) {
        if self.history.is_empty() {
            return;
        }
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

    pub async fn execute(&mut self) {
        let cmd = self.input.trim().to_string();
        if cmd.is_empty() {
            return;
        }

        if self.history.last().map(|h| h.as_str()) != Some(&cmd) {
            self.history.push(cmd.clone());
        }
        if self.history.len() > 500 {
            self.history.drain(..self.history.len() - 500);
        }

        self.history_index = None;
        self.input.clear();
        self.cursor_pos = 0;
        self.completer.dismiss();
        self.scroll_offset = 0;

        // Handle "clear" built-in
        if cmd == "clear" {
            self.entries.clear();
            return;
        }

        let entry_idx = self.entries.len();
        self.entries.push(CommandEntry {
            command: cmd.clone(),
            output_lines: Vec::new(),
            exit_code: None,
            is_running: true,
            _started_at: Instant::now(),
        });

        let tx = self.cmd_tx.clone();
        tokio::spawn(async move {
            let result = Command::new("sh")
                .arg("-c")
                .arg(&cmd)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .stdin(Stdio::null())
                .spawn();

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
                                let _ = tx_out.send((entry_idx, line, false));
                            }
                        })
                    });

                    let tx_err = tx.clone();
                    let stderr_handle = stderr.map(|err| {
                        tokio::spawn(async move {
                            let reader = BufReader::new(err);
                            let mut lines = reader.lines();
                            while let Ok(Some(line)) = lines.next_line().await {
                                let _ = tx_err.send((entry_idx, line, true));
                            }
                        })
                    });

                    let status = child.wait().await;
                    if let Some(h) = stdout_handle {
                        let _ = h.await;
                    }
                    if let Some(h) = stderr_handle {
                        let _ = h.await;
                    }

                    let exit_code = status.ok().and_then(|s| s.code()).unwrap_or(-1);
                    let _ = tx.send((entry_idx, format!("__EXIT__:{}", exit_code), false));
                }
                Err(e) => {
                    let _ = tx.send((entry_idx, format!("Failed to spawn: {}", e), true));
                    let _ = tx.send((entry_idx, "__EXIT__:-1".to_string(), false));
                }
            }
        });
    }

    pub fn total_output_lines(&self) -> usize {
        self.entries
            .iter()
            .map(|e| 1 + e.output_lines.len() + 1)
            .sum()
    }

    pub fn scroll_up(&mut self, amount: usize) {
        let max = self.total_output_lines().saturating_sub(1);
        self.scroll_offset = (self.scroll_offset + amount).min(max);
    }

    pub fn scroll_down(&mut self, amount: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(amount);
    }
}
