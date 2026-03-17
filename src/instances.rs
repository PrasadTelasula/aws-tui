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
    pub private_ip: String,
    pub instance_type: String,
    pub az: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum InstanceFocus {
    RegionList,
    InstanceList,
}

pub struct InstancesState {
    // Profile
    pub profiles: Vec<String>,
    pub active_profile_idx: usize,

    // Region
    pub region_idx: usize,
    pub regions: Vec<String>,
    pub region_dropdown_open: bool,

    // Instances
    pub instances: Vec<Ec2Instance>,
    pub selected_instance: usize,
    pub loading_instances: bool,
    pub last_error: Option<String>,

    // Focus
    pub focus: InstanceFocus,

    // Active SSM sessions opened in external terminals
    pub active_sessions: Vec<String>, // instance IDs with open terminals

    // Channel for instance fetch results
    fetch_tx: mpsc::UnboundedSender<Result<Vec<Ec2Instance>, String>>,
    fetch_rx: mpsc::UnboundedReceiver<Result<Vec<Ec2Instance>, String>>,
}

impl InstancesState {
    pub fn new() -> Self {
        let (fetch_tx, fetch_rx) = mpsc::unbounded_channel();

        Self {
            profiles: Vec::new(),
            active_profile_idx: 0,
            region_idx: 3, // us-west-2
            regions: REGIONS.iter().map(|s| s.to_string()).collect(),
            region_dropdown_open: false,
            instances: Vec::new(),
            selected_instance: 0,
            loading_instances: false,
            last_error: None,
            focus: InstanceFocus::InstanceList,
            active_sessions: Vec::new(),
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
            InstanceFocus::InstanceList => InstanceFocus::RegionList,
        };
    }

    /// Process incoming fetch results
    pub fn tick(&mut self) {
        while let Ok(result) = self.fetch_rx.try_recv() {
            self.loading_instances = false;
            match result {
                Ok(instances) => {
                    self.instances = instances;
                    self.selected_instance = 0;
                    self.last_error = None;
                }
                Err(e) => {
                    self.instances.clear();
                    self.last_error = Some(e);
                }
            }
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
        self.last_error = None;

        tokio::spawn(async move {
            let output = Command::new("aws")
                .args([
                    "ec2", "describe-instances",
                    "--region", &region,
                    "--profile", &profile,
                    "--filters", "Name=instance-state-name,Values=running",
                    "--query",
                    "Reservations[].Instances[].{InstanceId:InstanceId,Name:Tags[?Key==`Name`]|[0].Value,PrivateIp:PrivateIpAddress,Type:InstanceType,AZ:Placement.AvailabilityZone}",
                    "--output", "json",
                ])
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .stdin(Stdio::null())
                .output()
                .await;

            match output {
                Ok(out) if out.status.success() => {
                    let stdout = String::from_utf8_lossy(&out.stdout);
                    let instances = parse_instances(&stdout);
                    let _ = tx.send(Ok(instances));
                }
                Ok(out) => {
                    let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
                    let _ = tx.send(Err(stderr));
                }
                Err(e) => {
                    let _ = tx.send(Err(format!("Failed to run aws cli: {}", e)));
                }
            }
        });
    }

    /// Open SSM session in a native terminal window/split
    pub fn open_ssm_terminal(&mut self) {
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
        let name = instance.name.clone();

        let ssm_cmd = format!(
            "aws ssm start-session --target {} --region {} --profile {}",
            instance_id, region, profile
        );

        let term_program = std::env::var("TERM_PROGRAM").unwrap_or_default();

        let script = if term_program == "iTerm.app" {
            // iTerm2: split pane vertically, set session name, run SSM
            let set_name = format!("SSM: {} ({})", name, instance_id);
            format!(
                "tell application \"iTerm2\"\n\
                    tell current session of current window\n\
                        set newSession to (split vertically with default profile)\n\
                        tell newSession\n\
                            delay 0.3\n\
                            write text \"{}\"\n\
                            set name to \"{}\"\n\
                        end tell\n\
                    end tell\n\
                end tell",
                ssm_cmd, set_name
            )
        } else {
            // Terminal.app: new window
            format!(
                "tell application \"Terminal\"\n\
                    do script \"{}\"\n\
                    activate\n\
                end tell",
                ssm_cmd
            )
        };

        if !self.active_sessions.contains(&instance_id) {
            self.active_sessions.push(instance_id);
        }

        tokio::spawn(async move {
            let _ = Command::new("osascript")
                .arg("-e")
                .arg(&script)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn();
        });
    }

    pub fn selected_instance(&self) -> Option<&Ec2Instance> {
        self.instances.get(self.selected_instance)
    }

    pub fn has_active_session(&self, instance_id: &str) -> bool {
        self.active_sessions.contains(&instance_id.to_string())
    }
}

fn parse_instances(json: &str) -> Vec<Ec2Instance> {
    let parsed: Vec<serde_json::Value> = match serde_json::from_str(json) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };

    let mut instances: Vec<Ec2Instance> = parsed
        .iter()
        .map(|v| Ec2Instance {
            instance_id: v["InstanceId"].as_str().unwrap_or("-").to_string(),
            name: v["Name"].as_str().unwrap_or("(no name)").to_string(),
            private_ip: v["PrivateIp"].as_str().unwrap_or("-").to_string(),
            instance_type: v["Type"].as_str().unwrap_or("-").to_string(),
            az: v["AZ"].as_str().unwrap_or("-").to_string(),
        })
        .collect();

    instances.sort_by(|a, b| a.name.cmp(&b.name));
    instances
}
