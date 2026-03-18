use regex::Regex;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, PartialEq)]
pub enum AliasKind {
    SsoLogin { session_name: String },
    SsmSession {
        target: String,
        document: Option<String>,
        local_port: Option<String>,
        remote_port: Option<String>,
        host: Option<String>,
    },
    Other,
}

#[derive(Debug, Clone)]
pub struct Alias {
    pub name: String,
    pub command: String,
    pub kind: AliasKind,
    pub group: String,
}

impl Alias {}


fn parse_ssm_parameters(params_str: &str) -> HashMap<String, Vec<String>> {
    let mut result = HashMap::new();

    // Try to parse as JSON-like structure
    // The alias parameters look like: {"portNumber":["1111"],"host":["db.example.com"],...}
    // But they may have escaped quotes or other shell artifacts
    let cleaned = params_str
        .replace('\\', "")
        .replace("'{", "{")
        .replace("}'", "}");

    if let Ok(parsed) = serde_json::from_str::<HashMap<String, serde_json::Value>>(&cleaned) {
        for (key, value) in parsed {
            match value {
                serde_json::Value::Array(arr) => {
                    let strings: Vec<String> = arr
                        .iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect();
                    result.insert(key, strings);
                }
                serde_json::Value::String(s) => {
                    result.insert(key, vec![s]);
                }
                _ => {}
            }
        }
    }

    result
}

pub fn parse_alias_file(path: &Path) -> Vec<Alias> {
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };

    parse_aliases(&content)
}

pub fn parse_aliases(content: &str) -> Vec<Alias> {
    let alias_re = Regex::new(r#"^\s*alias\s+([a-zA-Z0-9_-]+)\s*=\s*'(.+?)'\s*$"#).unwrap();
    let group_re = Regex::new(r"(?i)#\s*group:\s*(.+)").unwrap();
    let mut aliases = Vec::new();
    let mut current_group = "Other".to_string();

    for line in content.lines() {
        let line = line.trim();

        if line.is_empty() {
            continue;
        }

        if let Some(caps) = group_re.captures(line) {
            let raw = caps[1].trim();
            // Strip any trailing metadata like "Type: SSO" — kind is auto-detected
            let type_re = Regex::new(r"(?i)\s+type\s*:.*$").unwrap();
            current_group = type_re.replace(raw, "").trim().to_string();
            continue;
        }

        if line.starts_with('#') {
            continue;
        }

        if let Some(caps) = alias_re.captures(line) {
            let name = caps[1].to_string();
            let command = caps[2].to_string();
            let kind = classify_command(&command);

            aliases.push(Alias {
                name,
                command,
                kind,
                group: current_group.clone(),
            });
        }
    }

    aliases
}

fn classify_command(command: &str) -> AliasKind {
    if command.contains("aws sso login") {
        let session_re = Regex::new(r"--sso-session\s+(\S+)").unwrap();
        let session_name = session_re
            .captures(command)
            .map(|c| c[1].to_string())
            .unwrap_or_default();
        AliasKind::SsoLogin { session_name }
    } else if command.contains("aws ssm start-session") {
        let target_re = Regex::new(r"--target\s+(\S+)").unwrap();
        let doc_re = Regex::new(r"--document-name\s+(\S+)").unwrap();
        let params_re = Regex::new(r"--parameters\s+(.+)$").unwrap();

        let target = target_re
            .captures(command)
            .map(|c| c[1].to_string())
            .unwrap_or_default();
        let document = doc_re.captures(command).map(|c| c[1].to_string());

        let mut local_port = None;
        let mut remote_port = None;
        let mut host = None;

        if let Some(params_cap) = params_re.captures(command) {
            let params = parse_ssm_parameters(&params_cap[1]);
            local_port = params
                .get("localPortNumber")
                .and_then(|v| v.first())
                .cloned();
            remote_port = params.get("portNumber").and_then(|v| v.first()).cloned();
            host = params.get("host").and_then(|v| v.first()).cloned();
        }

        AliasKind::SsmSession {
            target,
            document,
            local_port,
            remote_port,
            host,
        }
    } else {
        AliasKind::Other
    }
}
