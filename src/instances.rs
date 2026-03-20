use std::process::Stdio;
use tokio::process::Command;
use tokio::sync::mpsc;

// ─── Instance info popup ─────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum InfoTab {
    Human,
    Json,
}

pub struct InstanceInfoPopup {
    pub loading: bool,
    pub tab: InfoTab,
    pub scroll: u16,
    pub human_lines: Vec<String>,
    pub json_lines: Vec<String>,
    // Search within popup
    pub search_query: String,
    pub search_active: bool,
    pub search_matches: Vec<usize>,  // absolute line indices with a match
    pub search_match_idx: usize,     // which match is "current"
}

impl InstanceInfoPopup {
    pub fn new_loading() -> Self {
        Self {
            loading: true, tab: InfoTab::Human, scroll: 0,
            human_lines: Vec::new(), json_lines: Vec::new(),
            search_query: String::new(), search_active: false,
            search_matches: Vec::new(), search_match_idx: 0,
        }
    }

    pub fn lines(&self) -> &Vec<String> {
        match self.tab {
            InfoTab::Human => &self.human_lines,
            InfoTab::Json  => &self.json_lines,
        }
    }

    pub fn scroll_down(&mut self, n: u16) {
        let max = self.lines().len().saturating_sub(1) as u16;
        self.scroll = (self.scroll + n).min(max);
    }

    pub fn scroll_up(&mut self, n: u16) {
        self.scroll = self.scroll.saturating_sub(n);
    }

    pub fn update_search(&mut self) {
        if self.search_query.is_empty() {
            self.search_matches.clear();
            self.search_match_idx = 0;
            return;
        }
        let query = self.search_query.to_lowercase();
        self.search_matches = self.lines().iter().enumerate()
            .filter(|(_, l)| l.to_lowercase().contains(&query))
            .map(|(i, _)| i)
            .collect();
        self.search_match_idx = 0;
        self.jump_to_current_match();
    }

    pub fn next_match(&mut self) {
        if self.search_matches.is_empty() { return; }
        self.search_match_idx = (self.search_match_idx + 1) % self.search_matches.len();
        self.jump_to_current_match();
    }

    pub fn prev_match(&mut self) {
        if self.search_matches.is_empty() { return; }
        self.search_match_idx = if self.search_match_idx == 0 {
            self.search_matches.len() - 1
        } else {
            self.search_match_idx - 1
        };
        self.jump_to_current_match();
    }

    fn jump_to_current_match(&mut self) {
        if let Some(&line_idx) = self.search_matches.get(self.search_match_idx) {
            self.scroll = line_idx as u16;
        }
    }
}

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
    pub platform: String,  // "windows" or "linux"
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

    // Search
    pub search_query: String,
    pub search_active: bool,
    pub filtered_instances: Vec<usize>,

    // Instance info popup
    pub show_info_popup: bool,
    pub info_popup: Option<InstanceInfoPopup>,

    // Channel for instance fetch results
    fetch_tx: mpsc::UnboundedSender<Vec<Ec2Instance>>,
    fetch_rx: mpsc::UnboundedReceiver<Vec<Ec2Instance>>,

    // Channel for instance info popup results
    info_tx: mpsc::UnboundedSender<InstanceInfoPopup>,
    info_rx: mpsc::UnboundedReceiver<InstanceInfoPopup>,
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
        let (info_tx, info_rx) = mpsc::unbounded_channel();

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
            search_query: String::new(),
            search_active: false,
            filtered_instances: Vec::new(),
            show_info_popup: false,
            info_popup: None,
            fetch_tx,
            fetch_rx,
            info_tx,
            info_rx,
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
        if !self.filtered_instances.is_empty() {
            let pos = self.filtered_instances.iter().position(|&i| i == self.selected_instance).unwrap_or(0);
            self.selected_instance = self.filtered_instances[(pos + 1) % self.filtered_instances.len()];
        } else if !self.instances.is_empty() {
            self.selected_instance = (self.selected_instance + 1) % self.instances.len();
        }
    }

    pub fn prev_instance(&mut self) {
        if !self.filtered_instances.is_empty() {
            let pos = self.filtered_instances.iter().position(|&i| i == self.selected_instance).unwrap_or(0);
            let prev = if pos == 0 { self.filtered_instances.len() - 1 } else { pos - 1 };
            self.selected_instance = self.filtered_instances[prev];
        } else if !self.instances.is_empty() {
            self.selected_instance = if self.selected_instance == 0 {
                self.instances.len() - 1
            } else {
                self.selected_instance - 1
            };
        }
    }

    pub fn update_instance_search(&mut self) {
        if self.search_query.is_empty() {
            self.filtered_instances.clear();
            return;
        }
        let query = self.search_query.to_lowercase();
        self.filtered_instances = self.instances.iter().enumerate()
            .filter(|(_, inst)| {
                inst.name.to_lowercase().contains(&query)
                    || inst.instance_id.to_lowercase().contains(&query)
                    || inst.private_ip.contains(&query)
            })
            .map(|(i, _)| i)
            .collect();
        if !self.filtered_instances.is_empty()
            && !self.filtered_instances.contains(&self.selected_instance)
        {
            self.selected_instance = self.filtered_instances[0];
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

        // Process instance info popup results
        while let Ok(popup) = self.info_rx.try_recv() {
            self.info_popup = Some(popup);
        }

        // Process instance fetch results
        while let Ok(instances) = self.fetch_rx.try_recv() {
            self.instances = instances;
            self.selected_instance = 0;
            self.loading_instances = false;
            self.search_query.clear();
            self.search_active = false;
            self.filtered_instances.clear();
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
                    "--query", "Reservations[].Instances[].{InstanceId:InstanceId,Name:Tags[?Key==`Name`]|[0].Value,State:State.Name,PrivateIp:PrivateIpAddress,Type:InstanceType,Platform:Platform}",
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

    // ── Instance info popup ──────────────────────────────────────────

    pub fn fetch_instance_info(&mut self) {
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
        let tx = self.info_tx.clone();

        self.show_info_popup = true;
        self.info_popup = Some(InstanceInfoPopup::new_loading());

        tokio::spawn(async move {
            let output = Command::new("aws")
                .args([
                    "ec2", "describe-instances",
                    "--instance-ids", &instance_id,
                    "--region", &region,
                    "--profile", &profile,
                    "--output", "json",
                ])
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .stdin(Stdio::null())
                .output()
                .await;

            let popup = match output {
                Ok(out) if out.status.success() => {
                    let raw = String::from_utf8_lossy(&out.stdout).to_string();
                    let human = format_instance_human(&raw);
                    let json_lines = format_json_pretty(&raw);
                    InstanceInfoPopup {
                        loading: false, tab: InfoTab::Human, scroll: 0,
                        human_lines: human, json_lines,
                        search_query: String::new(), search_active: false,
                        search_matches: Vec::new(), search_match_idx: 0,
                    }
                }
                Ok(out) => {
                    let err = String::from_utf8_lossy(&out.stderr).to_string();
                    InstanceInfoPopup {
                        loading: false, tab: InfoTab::Human, scroll: 0,
                        human_lines: vec![format!("Error: {}", err)],
                        json_lines: vec![format!("Error: {}", err)],
                        search_query: String::new(), search_active: false,
                        search_matches: Vec::new(), search_match_idx: 0,
                    }
                }
                Err(e) => InstanceInfoPopup {
                    loading: false, tab: InfoTab::Human, scroll: 0,
                    human_lines: vec![format!("Error: {}", e)],
                    json_lines: vec![format!("Error: {}", e)],
                    search_query: String::new(), search_active: false,
                    search_matches: Vec::new(), search_match_idx: 0,
                },
            };
            let _ = tx.send(popup);
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
        .map(|v| {
            // Platform is "windows" for Windows; absent/null for Linux
            let platform = match v["Platform"].as_str() {
                Some(p) if p.to_lowercase().contains("windows") => "windows",
                _ => "linux",
            };
            Ec2Instance {
                instance_id: v["InstanceId"].as_str().unwrap_or("-").to_string(),
                name: v["Name"].as_str().unwrap_or("(no name)").to_string(),
                _state: v["State"].as_str().unwrap_or("-").to_string(),
                private_ip: v["PrivateIp"].as_str().unwrap_or("-").to_string(),
                _instance_type: v["Type"].as_str().unwrap_or("-").to_string(),
                platform: platform.to_string(),
            }
        })
        .collect()
}

/// Format the raw describe-instances JSON into human-readable lines.
fn format_instance_human(json: &str) -> Vec<String> {
    let root: serde_json::Value = match serde_json::from_str(json) {
        Ok(v) => v,
        Err(e) => return vec![format!("Parse error: {}", e)],
    };

    let inst = match root["Reservations"][0]["Instances"][0].as_object() {
        Some(o) => o,
        None => return vec!["No instance data found.".to_string()],
    };

    let mut lines: Vec<String> = Vec::new();

    let str_val = |v: &serde_json::Value| v.as_str().unwrap_or("-").to_string();

    // ── Identity ─────────────────────────────────────────────────────
    lines.push("── Identity ─────────────────────────────────────────".to_string());
    lines.push(format!("  Instance ID      {}", str_val(&inst["InstanceId"])));
    lines.push(format!("  Instance Type    {}", str_val(&inst["InstanceType"])));
    lines.push(format!("  Architecture     {}", str_val(&inst["Architecture"])));
    let platform = inst.get("Platform").and_then(|v| v.as_str()).unwrap_or("Linux");
    lines.push(format!("  Platform         {}", platform));
    lines.push(format!("  AMI ID           {}", str_val(&inst["ImageId"])));
    lines.push(String::new());

    // ── State ─────────────────────────────────────────────────────────
    lines.push("── State ────────────────────────────────────────────".to_string());
    lines.push(format!("  State            {}", str_val(&inst["State"]["Name"])));
    lines.push(format!("  Launch Time      {}", str_val(&inst["LaunchTime"])));
    if let Some(reason) = inst.get("StateTransitionReason").and_then(|v| v.as_str()) {
        if !reason.is_empty() {
            lines.push(format!("  State Reason     {}", reason));
        }
    }
    lines.push(String::new());

    // ── Network ────────────────────────────────────────────────────────
    lines.push("── Network ──────────────────────────────────────────".to_string());
    lines.push(format!("  Private IP       {}", str_val(&inst["PrivateIpAddress"])));
    lines.push(format!("  Private DNS      {}", str_val(&inst["PrivateDnsName"])));
    if let Some(ip) = inst.get("PublicIpAddress").and_then(|v| v.as_str()) {
        lines.push(format!("  Public IP        {}", ip));
    }
    if let Some(dns) = inst.get("PublicDnsName").and_then(|v| v.as_str()) {
        if !dns.is_empty() {
            lines.push(format!("  Public DNS       {}", dns));
        }
    }
    lines.push(format!("  VPC ID           {}", str_val(&inst["VpcId"])));
    lines.push(format!("  Subnet ID        {}", str_val(&inst["SubnetId"])));
    if let Some(az) = inst["Placement"]["AvailabilityZone"].as_str() {
        lines.push(format!("  Availability Zone {}", az));
    }
    lines.push(String::new());

    // ── Security ────────────────────────────────────────────────────────
    lines.push("── Security ─────────────────────────────────────────".to_string());
    if let Some(key) = inst.get("KeyName").and_then(|v| v.as_str()) {
        lines.push(format!("  Key Pair         {}", key));
    }
    if let Some(arn) = inst["IamInstanceProfile"]["Arn"].as_str() {
        // Show just the role name from the ARN
        let role = arn.split('/').last().unwrap_or(arn);
        lines.push(format!("  IAM Role         {}", role));
    }
    if let Some(sgs) = inst.get("SecurityGroups").and_then(|v| v.as_array()) {
        for sg in sgs {
            lines.push(format!("  Security Group   {} ({})",
                str_val(&sg["GroupId"]),
                str_val(&sg["GroupName"])));
        }
    }
    lines.push(String::new());

    // ── Compute ────────────────────────────────────────────────────────
    lines.push("── Compute ──────────────────────────────────────────".to_string());
    if let (Some(cores), Some(threads)) = (
        inst["CpuOptions"]["CoreCount"].as_u64(),
        inst["CpuOptions"]["ThreadsPerCore"].as_u64(),
    ) {
        lines.push(format!("  vCPUs            {}", cores * threads));
        lines.push(format!("  CPU Cores        {}  ({} thread(s)/core)", cores, threads));
    }
    if let Some(ebs) = inst.get("EbsOptimized").and_then(|v| v.as_bool()) {
        lines.push(format!("  EBS Optimized    {}", if ebs { "yes" } else { "no" }));
    }
    if let Some(mon) = inst["Monitoring"]["State"].as_str() {
        lines.push(format!("  Monitoring       {}", mon));
    }
    lines.push(String::new());

    // ── Storage ────────────────────────────────────────────────────────
    if let Some(vols) = inst.get("BlockDeviceMappings").and_then(|v| v.as_array()) {
        if !vols.is_empty() {
            lines.push("── Storage ──────────────────────────────────────────".to_string());
            for vol in vols {
                let dev  = str_val(&vol["DeviceName"]);
                let vid  = str_val(&vol["Ebs"]["VolumeId"]);
                let del  = vol["Ebs"]["DeleteOnTermination"].as_bool().unwrap_or(false);
                lines.push(format!("  {} → {}  (delete-on-term: {})", dev, vid, if del { "yes" } else { "no" }));
            }
            lines.push(String::new());
        }
    }

    // ── Tags ────────────────────────────────────────────────────────────
    if let Some(tags) = inst.get("Tags").and_then(|v| v.as_array()) {
        if !tags.is_empty() {
            lines.push("── Tags ─────────────────────────────────────────────".to_string());
            for tag in tags {
                lines.push(format!("  {:<24} {}",
                    str_val(&tag["Key"]),
                    str_val(&tag["Value"])));
            }
            lines.push(String::new());
        }
    }

    lines
}

/// Pretty-print JSON, one key-value pair per line with indentation.
fn format_json_pretty(json: &str) -> Vec<String> {
    match serde_json::from_str::<serde_json::Value>(json) {
        Ok(v) => match serde_json::to_string_pretty(&v) {
            Ok(pretty) => pretty.lines().map(|l| l.to_string()).collect(),
            Err(_) => json.lines().map(|l| l.to_string()).collect(),
        },
        Err(_) => json.lines().map(|l| l.to_string()).collect(),
    }
}
