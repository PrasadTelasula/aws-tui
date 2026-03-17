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
    pub region_dropdown_open: bool,

    // SSM connection (right panel) — PTY-based
    pub ssm_status: SsmConnectionStatus,
    pub pty_parser: Option<vt100::Parser>,
    pty_writer: Option<Box<dyn std::io::Write + Send>>,
    pty_master: Option<Box<dyn portable_pty::MasterPty + Send>>,
    pub pty_size: (u16, u16), // (rows, cols)
    pty_bytes_tx: mpsc::UnboundedSender<Vec<u8>>,
    pty_bytes_rx: mpsc::UnboundedReceiver<Vec<u8>>,

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
        let (pty_bytes_tx, pty_bytes_rx) = mpsc::unbounded_channel();
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
            ssm_status: SsmConnectionStatus::Disconnected,
            pty_parser: None,
            pty_writer: None,
            pty_master: None,
            pty_size: (24, 80),
            pty_bytes_tx,
            pty_bytes_rx,
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

    /// Process incoming PTY bytes and instance fetch results
    pub fn tick(&mut self) {
        while let Ok(bytes) = self.pty_bytes_rx.try_recv() {
            if bytes.is_empty() {
                // EOF signal — child exited
                self.ssm_status = SsmConnectionStatus::Disconnected;
                self.pty_master = None;
                self.pty_writer = None;
                continue;
            }
            if let Some(ref mut parser) = self.pty_parser {
                parser.process(&bytes);
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

    /// Connect to the selected instance via SSM using a PTY
    pub fn connect_ssm(&mut self, rows: u16, cols: u16) {
        // Disconnect any existing session first
        self.disconnect_ssm();

        if self.instances.is_empty() {
            return;
        }

        let instance = &self.instances[self.selected_instance];
        let instance_id = instance.instance_id.clone();
        let profile = match self.active_profile() {
            Some(p) => p.to_string(),
            None => return,
        };
        let region = self.active_region().to_string();

        self.ssm_status = SsmConnectionStatus::Connecting;

        let pty_system = portable_pty::native_pty_system();
        let pair = match pty_system.openpty(portable_pty::PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        }) {
            Ok(p) => p,
            Err(e) => {
                self.ssm_status = SsmConnectionStatus::Error(e.to_string());
                return;
            }
        };

        let mut cmd = portable_pty::CommandBuilder::new("aws");
        cmd.args(["ssm", "start-session", "--target", &instance_id, "--region", &region, "--profile", &profile]);

        let child = match pair.slave.spawn_command(cmd) {
            Ok(c) => c,
            Err(e) => {
                self.ssm_status = SsmConnectionStatus::Error(e.to_string());
                return;
            }
        };

        let writer = match pair.master.take_writer() {
            Ok(w) => w,
            Err(e) => {
                self.ssm_status = SsmConnectionStatus::Error(e.to_string());
                return;
            }
        };

        let reader = match pair.master.try_clone_reader() {
            Ok(r) => r,
            Err(e) => {
                self.ssm_status = SsmConnectionStatus::Error(e.to_string());
                return;
            }
        };

        let tx = self.pty_bytes_tx.clone();
        std::thread::spawn(move || {
            let mut buf = [0u8; 4096];
            let mut reader = reader;
            loop {
                match std::io::Read::read(&mut reader, &mut buf) {
                    Ok(0) | Err(_) => {
                        let _ = tx.send(vec![]); // empty = EOF signal
                        break;
                    }
                    Ok(n) => {
                        let _ = tx.send(buf[..n].to_vec());
                    }
                }
            }
            drop(child); // wait for child implicitly
        });

        self.pty_parser = Some(vt100::Parser::new(rows, cols, 1000));
        self.pty_writer = Some(writer);
        self.pty_master = Some(pair.master);
        self.pty_size = (rows, cols);
        self.ssm_status = SsmConnectionStatus::Connected;
    }

    /// Disconnect SSM session
    pub fn disconnect_ssm(&mut self) {
        // Dropping the master sends HUP to slave → child exits → reader thread gets EOF
        self.pty_master = None;
        self.pty_writer = None;
        self.pty_parser = None;
        self.ssm_status = SsmConnectionStatus::Disconnected;
    }

    /// Write raw bytes to the PTY
    pub fn write_input(&mut self, bytes: &[u8]) {
        if let Some(ref mut writer) = self.pty_writer {
            let _ = std::io::Write::write_all(writer, bytes);
            let _ = std::io::Write::flush(writer);
        }
    }

    /// Resize the PTY and parser
    pub fn resize_pty(&mut self, rows: u16, cols: u16) {
        if self.pty_size == (rows, cols) {
            return;
        }
        self.pty_size = (rows, cols);
        if let Some(ref master) = self.pty_master {
            let _ = master.resize(portable_pty::PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            });
        }
        if let Some(ref mut parser) = self.pty_parser {
            parser.set_size(rows, cols);
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
