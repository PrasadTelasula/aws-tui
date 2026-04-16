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
static SSM_PROFILE_RE: OnceLock<Regex> = OnceLock::new();
static SSM_REGION_RE: OnceLock<Regex> = OnceLock::new();

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
    let mut subgroup: Option<String> = None;

    for raw in content.lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(caps) = group_re().captures(line) {
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
            out.push(Alias {
                name,
                profile: extract_profile(&command),
                region: extract_region(&command),
                target: extract_target(&command),
                command,
                kind,
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
    let re = SSM_TARGET_RE.get_or_init(|| Regex::new(r"--target\s+(\S+)").unwrap());
    re.captures(command).map(|c| c[1].trim_matches('"').to_string())
}

fn extract_profile(command: &str) -> Option<String> {
    let re = SSM_PROFILE_RE.get_or_init(|| {
        Regex::new(r"--profile\s+(\S+)|--sso-session\s+(\S+)").unwrap()
    });
    re.captures(command).and_then(|c| {
        c.get(1).or_else(|| c.get(2)).map(|m| m.as_str().to_string())
    })
}

fn extract_region(command: &str) -> Option<String> {
    let re = SSM_REGION_RE.get_or_init(|| Regex::new(r"--region\s+(\S+)").unwrap());
    re.captures(command).map(|c| c[1].to_string())
}

#[allow(dead_code)]
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

/// Resolution order:
///   1. Explicit `path` argument (e.g. file picker)
///   2. AWS_TUI_ALIASES env var
///   3. ~/.zsh_aliases, ~/.bash_aliases, ~/.aliases
///   4. Bundled ./sample_aliases (relative to CWD or app dir)
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
