use std::process::Stdio;
use tokio::process::Command;
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
    Connected,
}

/// One live PTY session to a single EC2 instance.
pub struct SsmSession {
    pub instance_id: String,
    pub instance_name: String,
    pub parser: vt100::Parser,
    pub status: SsmConnectionStatus,
    writer: Box<dyn std::io::Write + Send>,
    master: Box<dyn portable_pty::MasterPty + Send>,
    bytes_rx: mpsc::UnboundedReceiver<Vec<u8>>,
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
    pub region_dropdown_open: bool,

    // SSM sessions (right panel) — one PTY per connected instance
    pub ssm_sessions: Vec<SsmSession>,
    pub active_session_idx: usize,
    pub pty_size: (u16, u16),        // shared panel size used for new sessions + resize
    pub last_error: Option<String>,  // last connection error to show in hint area

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
            region_dropdown_open: false,
            ssm_sessions: Vec::new(),
            active_session_idx: 0,
            pty_size: (24, 80),
            last_error: None,
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

    // ── Session navigation ──────────────────────────────────────────

    pub fn active_session(&self) -> Option<&SsmSession> {
        self.ssm_sessions.get(self.active_session_idx)
    }

    pub fn next_session(&mut self) {
        if self.ssm_sessions.len() > 1 {
            self.active_session_idx = (self.active_session_idx + 1) % self.ssm_sessions.len();
        }
    }

    pub fn prev_session(&mut self) {
        if self.ssm_sessions.len() > 1 {
            self.active_session_idx = if self.active_session_idx == 0 {
                self.ssm_sessions.len() - 1
            } else {
                self.active_session_idx - 1
            };
        }
    }

    // ── Tick ────────────────────────────────────────────────────────

    /// Drain PTY output from every session into their parsers.
    pub fn tick(&mut self) {
        for session in &mut self.ssm_sessions {
            while let Ok(bytes) = session.bytes_rx.try_recv() {
                if bytes.is_empty() {
                    session.status = SsmConnectionStatus::Disconnected;
                } else {
                    session.parser.process(&bytes);
                }
            }
        }

        // Process instance fetch results
        while let Ok(instances) = self.fetch_rx.try_recv() {
            self.instances = instances;
            self.selected_instance = 0;
            self.loading_instances = false;
        }
    }

    // ── Instance fetching ───────────────────────────────────────────

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

    // ── SSM PTY sessions ────────────────────────────────────────────

    /// Connect to the selected instance. If already connected, switch to that session.
    pub fn connect_ssm(&mut self, rows: u16, cols: u16) {
        if self.instances.is_empty() {
            return;
        }

        let instance = &self.instances[self.selected_instance];
        let instance_id = instance.instance_id.clone();
        let instance_name = instance.name.clone();

        // Already connected to this instance — just switch focus to it.
        if let Some(idx) = self.ssm_sessions.iter().position(|s| s.instance_id == instance_id) {
            self.active_session_idx = idx;
            self.last_error = None;
            return;
        }

        let profile = match self.active_profile() {
            Some(p) => p.to_string(),
            None => return,
        };
        let region = self.active_region().to_string();

        let pty_system = portable_pty::native_pty_system();
        let pair = match pty_system.openpty(portable_pty::PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        }) {
            Ok(p) => p,
            Err(e) => {
                self.last_error = Some(e.to_string());
                return;
            }
        };

        let mut cmd = portable_pty::CommandBuilder::new("aws");
        cmd.args(["ssm", "start-session",
            "--target", &instance_id,
            "--region", &region,
            "--profile", &profile,
        ]);

        let child = match pair.slave.spawn_command(cmd) {
            Ok(c) => c,
            Err(e) => {
                self.last_error = Some(e.to_string());
                return;
            }
        };

        let writer = match pair.master.take_writer() {
            Ok(w) => w,
            Err(e) => {
                self.last_error = Some(e.to_string());
                return;
            }
        };

        let reader = match pair.master.try_clone_reader() {
            Ok(r) => r,
            Err(e) => {
                self.last_error = Some(e.to_string());
                return;
            }
        };

        let (bytes_tx, bytes_rx) = mpsc::unbounded_channel::<Vec<u8>>();
        std::thread::spawn(move || {
            let mut buf = [0u8; 4096];
            let mut reader = reader;
            loop {
                match std::io::Read::read(&mut reader, &mut buf) {
                    Ok(0) | Err(_) => {
                        let _ = bytes_tx.send(vec![]); // EOF signal
                        break;
                    }
                    Ok(n) => {
                        let _ = bytes_tx.send(buf[..n].to_vec());
                    }
                }
            }
            drop(child);
        });

        self.ssm_sessions.push(SsmSession {
            instance_id,
            instance_name,
            parser: vt100::Parser::new(rows, cols, 1000),
            status: SsmConnectionStatus::Connected,
            writer,
            master: pair.master,
            bytes_rx,
        });
        self.active_session_idx = self.ssm_sessions.len() - 1;
        self.pty_size = (rows, cols);
        self.last_error = None;
    }

    /// Disconnect the currently active session and remove it.
    pub fn disconnect_ssm(&mut self) {
        if self.ssm_sessions.is_empty() {
            return;
        }
        // Dropping the session drops master → HUP → child exits → reader thread exits.
        self.ssm_sessions.remove(self.active_session_idx);
        if !self.ssm_sessions.is_empty() && self.active_session_idx >= self.ssm_sessions.len() {
            self.active_session_idx = self.ssm_sessions.len() - 1;
        }
    }

    /// Write raw input bytes to the active session's PTY.
    pub fn write_input(&mut self, bytes: &[u8]) {
        if let Some(session) = self.ssm_sessions.get_mut(self.active_session_idx) {
            if session.status == SsmConnectionStatus::Connected {
                let _ = std::io::Write::write_all(&mut session.writer, bytes);
                let _ = std::io::Write::flush(&mut session.writer);
            }
        }
    }

    /// Resize all session PTYs and parsers to match the current panel size.
    pub fn resize_pty(&mut self, rows: u16, cols: u16) {
        if self.pty_size == (rows, cols) {
            return;
        }
        self.pty_size = (rows, cols);
        for session in &mut self.ssm_sessions {
            let _ = session.master.resize(portable_pty::PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            });
            session.parser.set_size(rows, cols);
        }
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
