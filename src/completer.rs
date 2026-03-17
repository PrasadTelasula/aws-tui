use std::time::Instant;
use tokio::process::Command;

#[derive(Debug, Clone)]
pub struct CompletionResult {
    pub suggestions: Vec<String>,
    pub _prefix: String,
    pub prefix_start: usize,
}

pub struct Completer {
    last_queried_input: String,
    last_keystroke: Option<Instant>,
    debounce_ms: u64,
    in_flight: bool,
    pub cached_result: Option<CompletionResult>,
    pub selected_index: usize,
    pub visible: bool,
}

impl Completer {
    pub fn new() -> Self {
        Self {
            last_queried_input: String::new(),
            last_keystroke: None,
            debounce_ms: 150,
            in_flight: false,
            cached_result: None,
            selected_index: 0,
            visible: false,
        }
    }

    pub fn notify_keystroke(&mut self) {
        self.last_keystroke = Some(Instant::now());
    }

    pub fn should_query(&self, current_input: &str) -> bool {
        if current_input == self.last_queried_input {
            return false;
        }
        if self.in_flight {
            return false;
        }
        match self.last_keystroke {
            Some(t) => t.elapsed().as_millis() >= self.debounce_ms as u128,
            None => false,
        }
    }

    pub async fn query(&mut self, input: &str) {
        if input.trim().is_empty() {
            self.cached_result = None;
            self.visible = false;
            return;
        }

        self.in_flight = true;
        self.last_queried_input = input.to_string();

        let output = Command::new("aws_completer")
            .env("COMP_LINE", input)
            .env("COMP_POINT", input.len().to_string())
            .output()
            .await;

        self.in_flight = false;

        match output {
            Ok(out) if out.status.success() => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                let suggestions: Vec<String> = stdout
                    .lines()
                    .filter(|l| !l.is_empty())
                    .map(|l| l.to_string())
                    .collect();

                let prefix_start = input.rfind(' ').map(|i| i + 1).unwrap_or(0);
                let _prefix = input[prefix_start..].to_string();

                self.visible = !suggestions.is_empty();
                self.selected_index = 0;
                self.cached_result = Some(CompletionResult {
                    suggestions,
                    _prefix,
                    prefix_start,
                });
            }
            _ => {
                self.cached_result = None;
                self.visible = false;
            }
        }
    }

    pub fn accept_selected(&mut self, current_input: &str) -> Option<String> {
        let result = self.cached_result.as_ref()?;
        let suggestion = result.suggestions.get(self.selected_index)?;
        let new_input = format!("{}{} ", &current_input[..result.prefix_start], suggestion);
        self.visible = false;
        self.cached_result = None;
        Some(new_input)
    }

    pub fn next(&mut self) {
        if let Some(ref result) = self.cached_result {
            if !result.suggestions.is_empty() {
                self.selected_index = (self.selected_index + 1) % result.suggestions.len();
            }
        }
    }

    pub fn prev(&mut self) {
        if let Some(ref result) = self.cached_result {
            if !result.suggestions.is_empty() {
                self.selected_index = if self.selected_index == 0 {
                    result.suggestions.len() - 1
                } else {
                    self.selected_index - 1
                };
            }
        }
    }

    pub fn dismiss(&mut self) {
        self.visible = false;
    }
}
