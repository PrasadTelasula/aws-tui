use std::process::Stdio;
use tokio::process::Command;
use tokio::sync::mpsc;

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
    pub ssm_managed: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum InstanceFocus {
    RegionList,
    InstanceList,
    CommandInput,
}

/// A command sent via SSM send-command
#[derive(Debug, Clone)]
pub struct SsmCommand {
    pub command: String,
    pub _instance_name: String,
    pub output: Vec<String>,
    pub status: SsmCommandStatus,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SsmCommandStatus {
    Running,
    Success,
    Failed,
}

pub struct InstancesState {
    pub profiles: Vec<String>,
    pub active_profile_idx: usize,

    pub region_idx: usize,
    pub regions: Vec<String>,
    pub region_dropdown_open: bool,

    pub instances: Vec<Ec2Instance>,
    pub selected_instance: usize,
    pub loading_instances: bool,
    pub last_error: Option<String>,

    pub focus: InstanceFocus,

    // Command execution
    pub cmd_input: String,
    pub cmd_cursor: usize,
    pub cmd_history: Vec<SsmCommand>,
    pub cmd_scroll_offset: usize,

    // Channels
    fetch_tx: mpsc::UnboundedSender<Result<Vec<Ec2Instance>, String>>,
    fetch_rx: mpsc::UnboundedReceiver<Result<Vec<Ec2Instance>, String>>,
    cmd_tx: mpsc::UnboundedSender<(usize, SsmCommandResult)>,
    cmd_rx: mpsc::UnboundedReceiver<(usize, SsmCommandResult)>,
}

enum SsmCommandResult {
    Output(Vec<String>),
    Error(String),
}

impl InstancesState {
    pub fn new() -> Self {
        let (fetch_tx, fetch_rx) = mpsc::unbounded_channel();
        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();

        Self {
            profiles: Vec::new(),
            active_profile_idx: 0,
            region_idx: 3,
            regions: REGIONS.iter().map(|s| s.to_string()).collect(),
            region_dropdown_open: false,
            instances: Vec::new(),
            selected_instance: 0,
            loading_instances: false,
            last_error: None,
            focus: InstanceFocus::InstanceList,
            cmd_input: String::new(),
            cmd_cursor: 0,
            cmd_history: Vec::new(),
            cmd_scroll_offset: 0,
            fetch_tx,
            fetch_rx,
            cmd_tx,
            cmd_rx,
        }
    }

    pub fn active_profile(&self) -> Option<&str> {
        self.profiles.get(self.active_profile_idx).map(|s| s.as_str())
    }

    pub fn active_region(&self) -> &str {
        &self.regions[self.region_idx]
    }

    pub fn selected_instance(&self) -> Option<&Ec2Instance> {
        self.instances.get(self.selected_instance)
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
        self.region_idx = if self.region_idx == 0 { self.regions.len() - 1 } else { self.region_idx - 1 };
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
            InstanceFocus::InstanceList => InstanceFocus::CommandInput,
            InstanceFocus::CommandInput => InstanceFocus::RegionList,
        };
    }

    // Input editing
    pub fn insert_char(&mut self, c: char) {
        self.cmd_input.insert(self.cmd_cursor, c);
        self.cmd_cursor += c.len_utf8();
    }
    pub fn backspace(&mut self) {
        if self.cmd_cursor > 0 {
            let prev = self.cmd_input[..self.cmd_cursor]
                .char_indices().last().map(|(i, _)| i).unwrap_or(0);
            self.cmd_input.remove(prev);
            self.cmd_cursor = prev;
        }
    }
    pub fn clear_input(&mut self) {
        self.cmd_input.clear();
        self.cmd_cursor = 0;
    }

    pub fn scroll_up(&mut self, n: usize) {
        let total = self.total_output_lines();
        self.cmd_scroll_offset = (self.cmd_scroll_offset + n).min(total.saturating_sub(1));
    }
    pub fn scroll_down(&mut self, n: usize) {
        self.cmd_scroll_offset = self.cmd_scroll_offset.saturating_sub(n);
    }

    pub fn total_output_lines(&self) -> usize {
        self.cmd_history.iter().map(|c| 1 + c.output.len() + 1).sum()
    }

    pub fn tick(&mut self) {
        // Fetch results
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

        // Command results
        while let Ok((idx, result)) = self.cmd_rx.try_recv() {
            if let Some(entry) = self.cmd_history.get_mut(idx) {
                match result {
                    SsmCommandResult::Output(lines) => {
                        entry.output = lines;
                        entry.status = SsmCommandStatus::Success;
                    }
                    SsmCommandResult::Error(e) => {
                        entry.output = vec![e];
                        entry.status = SsmCommandStatus::Failed;
                    }
                }
            }
        }
    }

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
            // Step 1: Fetch EC2 instances
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

            let mut instances = match output {
                Ok(out) if out.status.success() => {
                    let stdout = String::from_utf8_lossy(&out.stdout);
                    parse_instances(&stdout)
                }
                Ok(out) => {
                    let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
                    let _ = tx.send(Err(stderr));
                    return;
                }
                Err(e) => {
                    let _ = tx.send(Err(format!("Failed to run aws cli: {}", e)));
                    return;
                }
            };

            // Step 2: Check which instances are SSM-managed
            let ssm_output = Command::new("aws")
                .args([
                    "ssm", "describe-instance-information",
                    "--region", &region,
                    "--profile", &profile,
                    "--query", "InstanceInformationList[].InstanceId",
                    "--output", "json",
                ])
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .stdin(Stdio::null())
                .output()
                .await;

            if let Ok(out) = ssm_output {
                if out.status.success() {
                    let stdout = String::from_utf8_lossy(&out.stdout);
                    if let Ok(ids) = serde_json::from_str::<Vec<String>>(&stdout) {
                        let managed_set: std::collections::HashSet<&str> =
                            ids.iter().map(|s| s.as_str()).collect();
                        for inst in &mut instances {
                            inst.ssm_managed = managed_set.contains(inst.instance_id.as_str());
                        }
                    }
                }
            }
            // If SSM check fails, instances keep ssm_managed=false (unknown)

            let _ = tx.send(Ok(instances));
        });
    }

    /// Execute a command on the selected instance via SSM send-command
    pub fn execute_command(&mut self) {
        let cmd = self.cmd_input.trim().to_string();
        if cmd.is_empty() { return; }

        let instance = match self.selected_instance() {
            Some(i) => i.clone(),
            None => return,
        };

        if !instance.ssm_managed {
            self.cmd_input.clear();
            self.cmd_cursor = 0;
            self.cmd_scroll_offset = 0;
            self.cmd_history.push(SsmCommand {
                command: format!("[{}] {}", instance.name, cmd),
                _instance_name: instance.name.clone(),
                output: vec![
                    "Instance is not managed by SSM.".to_string(),
                    "Ensure the instance has:".to_string(),
                    "  1. SSM Agent installed and running".to_string(),
                    "  2. An IAM instance profile with AmazonSSMManagedInstanceCore policy".to_string(),
                    "  3. Network connectivity to SSM endpoints (or VPC endpoint)".to_string(),
                ],
                status: SsmCommandStatus::Failed,
            });
            return;
        }

        let profile = match self.active_profile() {
            Some(p) => p.to_string(),
            None => return,
        };
        let region = self.active_region().to_string();

        self.cmd_input.clear();
        self.cmd_cursor = 0;
        self.cmd_scroll_offset = 0;

        if cmd == "clear" {
            self.cmd_history.clear();
            return;
        }

        let entry_idx = self.cmd_history.len();
        self.cmd_history.push(SsmCommand {
            command: format!("[{}] {}", instance.name, cmd),
            _instance_name: instance.name.clone(),
            output: vec!["running…".to_string()],
            status: SsmCommandStatus::Running,
        });

        let tx = self.cmd_tx.clone();
        let instance_id = instance.instance_id.clone();

        tokio::spawn(async move {
            // Step 1: send-command
            let send_output = Command::new("aws")
                .args([
                    "ssm", "send-command",
                    "--instance-ids", &instance_id,
                    "--document-name", "AWS-RunShellScript",
                    "--parameters", &format!("commands=[\"{}\"]", cmd.replace('"', "\\\"")),
                    "--profile", &profile,
                    "--region", &region,
                    "--output", "json",
                ])
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .stdin(Stdio::null())
                .output()
                .await;

            let command_id = match send_output {
                Ok(out) if out.status.success() => {
                    let stdout = String::from_utf8_lossy(&out.stdout);
                    let parsed: serde_json::Value = match serde_json::from_str(&stdout) {
                        Ok(v) => v,
                        Err(e) => {
                            let _ = tx.send((entry_idx, SsmCommandResult::Error(
                                format!("Failed to parse send-command response: {}", e),
                            )));
                            return;
                        }
                    };
                    match parsed["Command"]["CommandId"].as_str() {
                        Some(id) => id.to_string(),
                        None => {
                            let _ = tx.send((entry_idx, SsmCommandResult::Error(
                                "No CommandId in response".to_string(),
                            )));
                            return;
                        }
                    }
                }
                Ok(out) => {
                    let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
                    let msg = if stderr.contains("AccessDeniedException") {
                        format!(
                            "{}\n\nRequired IAM permissions:\n  \
                             - ssm:SendCommand\n  \
                             - ssm:GetCommandInvocation\n\
                             Ensure your IAM role/user has these permissions \
                             for the target instance.",
                            stderr
                        )
                    } else {
                        stderr
                    };
                    let _ = tx.send((entry_idx, SsmCommandResult::Error(msg)));
                    return;
                }
                Err(e) => {
                    let _ = tx.send((entry_idx, SsmCommandResult::Error(
                        format!("Failed to run aws cli: {}", e),
                    )));
                    return;
                }
            };

            // Step 2: poll get-command-invocation until complete
            for _ in 0..60 {
                tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

                let get_output = Command::new("aws")
                    .args([
                        "ssm", "get-command-invocation",
                        "--command-id", &command_id,
                        "--instance-id", &instance_id,
                        "--profile", &profile,
                        "--region", &region,
                        "--output", "json",
                    ])
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .stdin(Stdio::null())
                    .output()
                    .await;

                match get_output {
                    Ok(out) if out.status.success() => {
                        let stdout = String::from_utf8_lossy(&out.stdout);
                        let parsed: serde_json::Value = match serde_json::from_str(&stdout) {
                            Ok(v) => v,
                            Err(_) => continue,
                        };

                        let status = parsed["Status"].as_str().unwrap_or("");

                        if status == "InProgress" || status == "Pending" || status == "Delayed" {
                            continue;
                        }

                        let std_out = parsed["StandardOutputContent"]
                            .as_str().unwrap_or("").to_string();
                        let std_err = parsed["StandardErrorContent"]
                            .as_str().unwrap_or("").to_string();

                        let mut lines: Vec<String> = Vec::new();
                        for line in std_out.lines() {
                            lines.push(line.to_string());
                        }
                        if !std_err.is_empty() {
                            for line in std_err.lines() {
                                lines.push(format!("[stderr] {}", line));
                            }
                        }
                        if lines.is_empty() {
                            lines.push("(no output)".to_string());
                        }

                        if status == "Success" {
                            let _ = tx.send((entry_idx, SsmCommandResult::Output(lines)));
                        } else {
                            let _ = tx.send((entry_idx, SsmCommandResult::Error(
                                format!("Command {}: {}", status, lines.join("\n")),
                            )));
                        }
                        return;
                    }
                    Ok(out) => {
                        let stderr = String::from_utf8_lossy(&out.stderr);
                        if stderr.contains("InvocationDoesNotExist") {
                            continue; // Not ready yet
                        }
                        let _ = tx.send((entry_idx, SsmCommandResult::Error(
                            stderr.trim().to_string(),
                        )));
                        return;
                    }
                    Err(e) => {
                        let _ = tx.send((entry_idx, SsmCommandResult::Error(
                            format!("Failed to get invocation: {}", e),
                        )));
                        return;
                    }
                }
            }

            let _ = tx.send((entry_idx, SsmCommandResult::Error(
                "Command timed out after 120s".to_string(),
            )));
        });
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
            ssm_managed: false, // Updated after SSM describe-instance-information check
        })
        .collect();

    instances.sort_by(|a, b| a.name.cmp(&b.name));
    instances
}
