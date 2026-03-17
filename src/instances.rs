use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::mpsc;

/// AWS regions
pub const REGIONS: &[&str] = &[
    "us-east-1", "us-east-2", "us-west-1", "us-west-2",
    "eu-west-1", "eu-west-2", "eu-west-3", "eu-central-1", "eu-north-1",
    "ap-southeast-1", "ap-southeast-2", "ap-northeast-1", "ap-northeast-2",
    "ap-south-1", "sa-east-1", "ca-central-1",
    "me-south-1", "af-south-1",
];

#[derive(Debug, Clone)]
pub struct Ec2Instance {
    pub instance_id: String,
    pub name: String,
    pub _state: String,
    pub private_ip: String,
    pub _instance_type: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SsmConnectionStatus {
    Disconnected,
    Connecting,
    Connected,
    Error(String),
}

pub struct InstancesState {
    // Profile
    pub profiles: Vec<String>,
    pub active_profile_idx: usize,

    // Region
    pub region_idx: usize,
    pub regions: Vec<String>,

    // Instances
    pub instances: Vec<Ec2Instance>,
    pub selected_instance: usize,
    pub loading_instances: bool,

    // Focus
    pub focus: InstanceFocus,

    // SSM connection (right panel)
    pub ssm_status: SsmConnectionStatus,
    pub ssm_output: Vec<String>,
    pub ssm_input: String,
    pub ssm_cursor: usize,
    pub ssm_scroll_offset: usize,
    ssm_child_stdin: Option<tokio::process::ChildStdin>,
    ssm_child: Option<Child>,

    // Channel for SSM output
    ssm_tx: mpsc::UnboundedSender<String>,
    ssm_rx: mpsc::UnboundedReceiver<String>,

    // Channel for instance fetch results
    fetch_tx: mpsc::UnboundedSender<Vec<Ec2Instance>>,
    fetch_rx: mpsc::UnboundedReceiver<Vec<Ec2Instance>>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum InstanceFocus {
    RegionList,
    InstanceList,
    SsmTerminal,
}

impl InstancesState {
    pub fn new() -> Self {
        let (ssm_tx, ssm_rx) = mpsc::unbounded_channel();
        let (fetch_tx, fetch_rx) = mpsc::unbounded_channel();

        Self {
            profiles: Vec::new(),
            active_profile_idx: 0,
            region_idx: 3, // us-west-2 default
            regions: REGIONS.iter().map(|s| s.to_string()).collect(),
            instances: Vec::new(),
            selected_instance: 0,
            loading_instances: false,
            focus: InstanceFocus::InstanceList,
            ssm_status: SsmConnectionStatus::Disconnected,
            ssm_output: Vec::new(),
            ssm_input: String::new(),
            ssm_cursor: 0,
            ssm_scroll_offset: 0,
            ssm_child_stdin: None,
            ssm_child: None,
            ssm_tx,
            ssm_rx,
            fetch_tx,
            fetch_rx,
        }
    }

    pub fn active_profile(&self) -> Option<&str> {
        self.profiles.get(self.active_profile_idx).map(|s| s.as_str())
    }

    pub fn active_region(&self) -> &str {
        &self.regions[self.region_idx]
    }

    pub fn next_profile(&mut self) {
        if !self.profiles.is_empty() {
            self.active_profile_idx = (self.active_profile_idx + 1) % self.profiles.len();
        }
    }

    pub fn prev_profile(&mut self) {
        if !self.profiles.is_empty() {
            self.active_profile_idx = if self.active_profile_idx == 0 {
                self.profiles.len() - 1
            } else {
                self.active_profile_idx - 1
            };
        }
    }

    pub fn next_region(&mut self) {
        self.region_idx = (self.region_idx + 1) % self.regions.len();
    }

    pub fn prev_region(&mut self) {
        self.region_idx = if self.region_idx == 0 {
            self.regions.len() - 1
        } else {
            self.region_idx - 1
        };
    }

    pub fn next_instance(&mut self) {
        if !self.instances.is_empty() {
            self.selected_instance = (self.selected_instance + 1) % self.instances.len();
        }
    }

    pub fn prev_instance(&mut self) {
        if !self.instances.is_empty() {
            self.selected_instance = if self.selected_instance == 0 {
                self.instances.len() - 1
            } else {
                self.selected_instance - 1
            };
        }
    }

    pub fn cycle_focus(&mut self) {
        self.focus = match self.focus {
            InstanceFocus::RegionList => InstanceFocus::InstanceList,
            InstanceFocus::InstanceList => InstanceFocus::SsmTerminal,
            InstanceFocus::SsmTerminal => InstanceFocus::RegionList,
        };
    }

    /// Process incoming data
    pub fn tick(&mut self) {
        // Process SSM output
        while let Ok(line) = self.ssm_rx.try_recv() {
            if line == "__SSM_EXIT__" {
                self.ssm_status = SsmConnectionStatus::Disconnected;
                self.ssm_output.push(">>> Session ended".to_string());
                self.ssm_child_stdin = None;
                continue;
            }
            if line.starts_with("__SSM_ERROR__:") {
                let err = line["__SSM_ERROR__:".len()..].to_string();
                self.ssm_status = SsmConnectionStatus::Error(err.clone());
                self.ssm_output.push(format!(">>> Error: {}", err));
                self.ssm_child_stdin = None;
                continue;
            }
            self.ssm_output.push(line);
            if self.ssm_output.len() > 1000 {
                self.ssm_output.drain(..self.ssm_output.len() - 1000);
            }
        }

        // Process fetch results
        while let Ok(instances) = self.fetch_rx.try_recv() {
            self.instances = instances;
            self.selected_instance = 0;
            self.loading_instances = false;
        }
    }

    /// Fetch instances for the current profile + region
    pub fn fetch_instances(&mut self) {
        let profile = match self.active_profile() {
            Some(p) => p.to_string(),
            None => return,
        };
        let region = self.active_region().to_string();
        let tx = self.fetch_tx.clone();

        self.loading_instances = true;
        self.instances.clear();

        tokio::spawn(async move {
            let output = Command::new("aws")
                .args([
                    "ec2", "describe-instances",
                    "--region", &region,
                    "--profile", &profile,
                    "--filters", "Name=instance-state-name,Values=running",
                    "--query", "Reservations[].Instances[].{InstanceId:InstanceId,Name:Tags[?Key==`Name`]|[0].Value,State:State.Name,PrivateIp:PrivateIpAddress,Type:InstanceType}",
                    "--output", "json",
                ])
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .stdin(Stdio::null())
                .output()
                .await;

            let instances = match output {
                Ok(out) if out.status.success() => {
                    let stdout = String::from_utf8_lossy(&out.stdout);
                    parse_instances(&stdout)
                }
                Ok(out) => {
                    let stderr = String::from_utf8_lossy(&out.stderr);
                    eprintln!("EC2 error: {}", stderr);
                    Vec::new()
                }
                Err(_) => Vec::new(),
            };

            let _ = tx.send(instances);
        });
    }

    /// Connect to the selected instance via SSM
    pub async fn connect_ssm(&mut self) {
        if self.instances.is_empty() {
            return;
        }

        // Disconnect existing session first
        self.disconnect_ssm().await;

        let instance = &self.instances[self.selected_instance];
        let instance_id = instance.instance_id.clone();
        let profile = match self.active_profile() {
            Some(p) => p.to_string(),
            None => return,
        };
        let region = self.active_region().to_string();

        self.ssm_status = SsmConnectionStatus::Connecting;
        self.ssm_output.clear();
        self.ssm_output.push(format!(
            ">>> Connecting to {} ({}) in {}...",
            instance.name, instance_id, region
        ));

        let mut child = match Command::new("aws")
            .args([
                "ssm", "start-session",
                "--target", &instance_id,
                "--region", &region,
                "--profile", &profile,
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(Stdio::piped())
            .spawn()
        {
            Ok(c) => c,
            Err(e) => {
                self.ssm_status = SsmConnectionStatus::Error(e.to_string());
                self.ssm_output.push(format!(">>> Failed: {}", e));
                return;
            }
        };

        let stdin = child.stdin.take();
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();

        self.ssm_child_stdin = stdin;
        self.ssm_status = SsmConnectionStatus::Connected;
        self.ssm_output.push(">>> Connected".to_string());

        // Read stdout
        let tx = self.ssm_tx.clone();
        if let Some(out) = stdout {
            let tx_out = tx.clone();
            tokio::spawn(async move {
                let reader = BufReader::new(out);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    let _ = tx_out.send(line);
                }
                let _ = tx_out.send("__SSM_EXIT__".to_string());
            });
        }

        // Read stderr
        if let Some(err) = stderr {
            let tx_err = tx.clone();
            tokio::spawn(async move {
                let reader = BufReader::new(err);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    let _ = tx_err.send(format!("[stderr] {}", line));
                }
            });
        }

        self.ssm_child = Some(child);
    }

    /// Disconnect SSM session
    pub async fn disconnect_ssm(&mut self) {
        if let Some(ref mut child) = self.ssm_child {
            let _ = child.kill().await;
        }
        self.ssm_child = None;
        self.ssm_child_stdin = None;
        self.ssm_status = SsmConnectionStatus::Disconnected;
    }

    /// Send a command to the SSM session
    pub async fn send_command(&mut self) {
        let cmd = self.ssm_input.trim().to_string();
        if cmd.is_empty() {
            return;
        }

        self.ssm_input.clear();
        self.ssm_cursor = 0;
        self.ssm_scroll_offset = 0;

        if let Some(ref mut stdin) = self.ssm_child_stdin {
            let _ = stdin.write_all(format!("{}\n", cmd).as_bytes()).await;
            let _ = stdin.flush().await;
        }
    }

    // Input editing for SSM terminal
    pub fn insert_char(&mut self, c: char) {
        self.ssm_input.insert(self.ssm_cursor, c);
        self.ssm_cursor += c.len_utf8();
    }

    pub fn backspace(&mut self) {
        if self.ssm_cursor > 0 {
            let prev = self.ssm_input[..self.ssm_cursor]
                .char_indices()
                .last()
                .map(|(i, _)| i)
                .unwrap_or(0);
            self.ssm_input.remove(prev);
            self.ssm_cursor = prev;
        }
    }

    pub fn _cursor_left(&mut self) {
        if self.ssm_cursor > 0 {
            self.ssm_cursor = self.ssm_input[..self.ssm_cursor]
                .char_indices()
                .last()
                .map(|(i, _)| i)
                .unwrap_or(0);
        }
    }

    pub fn _cursor_right(&mut self) {
        if self.ssm_cursor < self.ssm_input.len() {
            if let Some(c) = self.ssm_input[self.ssm_cursor..].chars().next() {
                self.ssm_cursor += c.len_utf8();
            }
        }
    }

    pub fn scroll_up(&mut self, n: usize) {
        let max = self.ssm_output.len().saturating_sub(1);
        self.ssm_scroll_offset = (self.ssm_scroll_offset + n).min(max);
    }

    pub fn scroll_down(&mut self, n: usize) {
        self.ssm_scroll_offset = self.ssm_scroll_offset.saturating_sub(n);
    }
}

fn parse_instances(json: &str) -> Vec<Ec2Instance> {
    let parsed: Vec<serde_json::Value> = match serde_json::from_str(json) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };

    parsed
        .iter()
        .map(|v| Ec2Instance {
            instance_id: v["InstanceId"].as_str().unwrap_or("-").to_string(),
            name: v["Name"].as_str().unwrap_or("(no name)").to_string(),
            _state: v["State"].as_str().unwrap_or("-").to_string(),
            private_ip: v["PrivateIp"].as_str().unwrap_or("-").to_string(),
            _instance_type: v["Type"].as_str().unwrap_or("-").to_string(),
        })
        .collect()
}
