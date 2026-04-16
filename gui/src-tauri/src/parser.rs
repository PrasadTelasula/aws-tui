use crate::model::{Alias, AliasKind};
use regex::Regex;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

static ALIAS_RE: OnceLock<Regex> = OnceLock::new();
static GROUP_RE: OnceLock<Regex> = OnceLock::new();
static SSO_SESSION_RE: OnceLock<Regex> = OnceLock::new();
static SSM_TARGET_RE: OnceLock<Regex> = OnceLock::new();
static PROFILE_RE: OnceLock<Regex> = OnceLock::new();
static REGION_RE: OnceLock<Regex> = OnceLock::new();
static SSM_DOC_RE: OnceLock<Regex> = OnceLock::new();
static SSM_PARAMS_RE: OnceLock<Regex> = OnceLock::new();

fn alias_re() -> &'static Regex {
    ALIAS_RE.get_or_init(|| {
        Regex::new(r#"^\s*alias\s+([a-zA-Z0-9_-]+)\s*=\s*'(.+?)'\s*$"#).unwrap()
    })
}

fn group_re() -> &'static Regex {
    GROUP_RE.get_or_init(|| {
        Regex::new(r"(?i)#\s*group:\s*([^,]+?)(?:\s+type\s*:\s*(.+))?$").unwrap()
    })
}

pub fn parse(content: &str) -> Vec<Alias> {
    let mut out = Vec::new();
    let mut group: Option<String> = None;
    let mut subgroup: Option<String> = None;

    for raw in content.lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(caps) = group_re().captures(line) {
            group = Some(caps[1].trim().to_string());
            subgroup = caps.get(2).map(|m| m.as_str().trim().to_string());
            continue;
        }
        if line.starts_with('#') {
            continue;
        }
        if let Some(caps) = alias_re().captures(line) {
            let name = caps[1].to_string();
            let command = caps[2].to_string();
            let is_iam = subgroup
                .as_deref()
                .map(|s| s.eq_ignore_ascii_case("iam"))
                .unwrap_or(false);
            let kind = if is_iam {
                AliasKind::IamProfile
            } else {
                classify(&command)
            };
            let ssm_params = if matches!(kind, AliasKind::SsmSession) {
                extract_ssm_params(&command)
            } else {
                SsmParams::default()
            };
            out.push(Alias {
                profile: extract_profile(&command),
                region: extract_region(&command),
                target: extract_target(&command),
                sso_session_name: if matches!(kind, AliasKind::SsoLogin) {
                    extract_sso_session(&command)
                } else {
                    None
                },
                ssm_document: ssm_params.document,
                ssm_local_port: ssm_params.local_port,
                ssm_remote_port: ssm_params.remote_port,
                ssm_host: ssm_params.host,
                name,
                command,
                kind,
                group: group.clone(),
                subgroup: subgroup.clone(),
            });
        }
    }
    out
}

fn classify(command: &str) -> AliasKind {
    if command.contains("aws sso login") {
        AliasKind::SsoLogin
    } else if command.contains("aws ssm start-session") {
        AliasKind::SsmSession
    } else {
        AliasKind::Other
    }
}

fn extract_target(command: &str) -> Option<String> {
    let re = SSM_TARGET_RE.get_or_init(|| Regex::new(r#"--target\s+"?([^"\s]+)"?"#).unwrap());
    re.captures(command).map(|c| c[1].to_string())
}

fn extract_profile(command: &str) -> Option<String> {
    let re = PROFILE_RE.get_or_init(|| Regex::new(r"--profile\s+(\S+)").unwrap());
    re.captures(command).map(|c| c[1].to_string())
}

fn extract_region(command: &str) -> Option<String> {
    let re = REGION_RE.get_or_init(|| Regex::new(r"--region\s+(\S+)").unwrap());
    re.captures(command).map(|c| c[1].to_string())
}

fn extract_sso_session(command: &str) -> Option<String> {
    let re = SSO_SESSION_RE.get_or_init(|| Regex::new(r"--sso-session\s+(\S+)").unwrap());
    re.captures(command).map(|c| c[1].to_string())
}

#[derive(Default)]
struct SsmParams {
    document: Option<String>,
    local_port: Option<String>,
    remote_port: Option<String>,
    host: Option<String>,
}

fn extract_ssm_params(command: &str) -> SsmParams {
    let doc_re = SSM_DOC_RE.get_or_init(|| Regex::new(r"--document-name\s+(\S+)").unwrap());
    let params_re = SSM_PARAMS_RE.get_or_init(|| Regex::new(r"--parameters\s+(.+)$").unwrap());

    let mut out = SsmParams::default();
    out.document = doc_re.captures(command).map(|c| c[1].to_string());
    if let Some(p) = params_re.captures(command) {
        let parsed = parse_ssm_parameters(&p[1]);
        out.local_port = parsed.get("localPortNumber").and_then(|v| v.first()).cloned();
        out.remote_port = parsed.get("portNumber").and_then(|v| v.first()).cloned();
        out.host = parsed.get("host").and_then(|v| v.first()).cloned();
    }
    out
}

fn parse_ssm_parameters(params_str: &str) -> HashMap<String, Vec<String>> {
    let cleaned = params_str
        .replace('\\', "")
        .replace("'{", "{")
        .replace("}'", "}");
    let mut result = HashMap::new();
    if let Ok(parsed) =
        serde_json::from_str::<HashMap<String, serde_json::Value>>(&cleaned)
    {
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

pub fn resolve_path(explicit: Option<&str>) -> Option<PathBuf> {
    if let Some(p) = explicit {
        let pb = PathBuf::from(p);
        if pb.is_file() {
            return Some(pb);
        }
    }
    if let Ok(env_path) = std::env::var("AWS_TUI_ALIASES") {
        let pb = PathBuf::from(env_path);
        if pb.is_file() {
            return Some(pb);
        }
    }
    if let Some(home) = dirs::home_dir() {
        for name in [".zsh_aliases", ".bash_aliases", ".aliases"] {
            let pb = home.join(name);
            if pb.is_file() {
                return Some(pb);
            }
        }
    }
    let candidates = [
        PathBuf::from("sample_aliases"),
        PathBuf::from("../sample_aliases"),
        PathBuf::from("../../sample_aliases"),
    ];
    for c in candidates {
        if c.is_file() {
            return Some(c);
        }
    }
    None
}

pub fn read_aliases(explicit: Option<&str>) -> Result<(PathBuf, Vec<Alias>), String> {
    let path = resolve_path(explicit)
        .ok_or_else(|| "No aliases file found (set AWS_TUI_ALIASES or use the file picker)".to_string())?;
    read_aliases_at(&path).map(|aliases| (path, aliases))
}

pub fn read_aliases_at(path: &Path) -> Result<Vec<Alias>, String> {
    let content = fs::read_to_string(path)
        .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;
    Ok(parse(&content))
}
