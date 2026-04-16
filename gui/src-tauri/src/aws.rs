//! Direct shell-outs to the `aws` CLI for STS, credential export, and EC2
//! tag-to-instance resolution.

use crate::model::CredentialInfo;
use regex::Regex;
use std::process::Stdio;
use tokio::process::Command;

pub enum StsCheckResult {
    Valid { account: String, arn: String },
    Expired { error: String },
    Error { error: String },
}

pub async fn check_sts_identity(profile: &str) -> StsCheckResult {
    let output = Command::new("aws")
        .args([
            "sts",
            "get-caller-identity",
            "--profile",
            profile,
            "--output",
            "json",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::null())
        .output()
        .await;

    match output {
        Ok(out) => {
            if out.status.success() {
                let stdout = String::from_utf8_lossy(&out.stdout);
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&stdout) {
                    let account = parsed
                        .get("Account")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown")
                        .to_string();
                    let arn = parsed
                        .get("Arn")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown")
                        .to_string();
                    StsCheckResult::Valid { account, arn }
                } else {
                    StsCheckResult::Valid {
                        account: "ok".into(),
                        arn: stdout.trim().to_string(),
                    }
                }
            } else {
                let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
                let lower = stderr.to_lowercase();
                if lower.contains("expired")
                    || lower.contains("not authorized")
                    || lower.contains("invalid")
                    || lower.contains("the sso session")
                    || lower.contains("token has expired")
                    || lower.contains("refresh failed")
                {
                    StsCheckResult::Expired { error: stderr }
                } else {
                    StsCheckResult::Error { error: stderr }
                }
            }
        }
        Err(e) => StsCheckResult::Error {
            error: format!("Failed to run aws cli: {e}"),
        },
    }
}

pub async fn fetch_credentials(profile: &str) -> Option<CredentialInfo> {
    let output = Command::new("aws")
        .args(["configure", "export-credentials", "--profile", profile])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .stdin(Stdio::null())
        .output()
        .await
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).ok()?;
    Some(CredentialInfo {
        access_key_id: json
            .get("AccessKeyId")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        secret_access_key: json
            .get("SecretAccessKey")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        session_token: json
            .get("SessionToken")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        expiration: json
            .get("Expiration")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
    })
}

pub fn resolve_profile_for_sso_session(session_name: &str) -> Option<String> {
    let config_path = dirs::home_dir()?.join(".aws/config");
    let content = std::fs::read_to_string(config_path).ok()?;
    let mut current_profile: Option<String> = None;
    let target = session_name.to_lowercase();

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            let inner = trimmed[1..trimmed.len() - 1].trim();
            if let Some(rest) = inner.strip_prefix("profile ") {
                current_profile = Some(rest.trim().to_string());
            } else if inner == "default" {
                current_profile = Some("default".to_string());
            } else {
                current_profile = None;
            }
            continue;
        }
        if let Some(ref profile) = current_profile {
            if let Some((key, value)) = trimmed.split_once('=') {
                let key = key.trim().to_lowercase();
                let value = value.trim().to_lowercase();
                if key == "sso_session" && value == target {
                    return Some(profile.clone());
                }
            }
        }
    }
    None
}

pub fn iam_profile_exists_in_credentials(profile_name: &str) -> bool {
    let creds_path = match dirs::home_dir() {
        Some(h) => h.join(".aws/credentials"),
        None => return false,
    };
    let content = match std::fs::read_to_string(creds_path) {
        Ok(c) => c,
        Err(_) => return false,
    };
    let target = format!("[{profile_name}]");
    content.lines().any(|line| line.trim() == target)
}

pub fn read_iam_credentials_from_file(profile_name: &str) -> Option<CredentialInfo> {
    let creds_path = dirs::home_dir()?.join(".aws/credentials");
    let content = std::fs::read_to_string(creds_path).ok()?;
    let target_section = format!("[{profile_name}]");
    let mut in_section = false;
    let mut access_key = String::new();
    let mut secret_key = String::new();

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            if in_section {
                break;
            }
            in_section = trimmed == target_section;
            continue;
        }
        if in_section {
            if let Some((key, value)) = trimmed.split_once('=') {
                match key.trim().to_lowercase().as_str() {
                    "aws_access_key_id" => access_key = value.trim().to_string(),
                    "aws_secret_access_key" => secret_key = value.trim().to_string(),
                    _ => {}
                }
            }
        }
    }
    if access_key.is_empty() || secret_key.is_empty() {
        return None;
    }
    Some(CredentialInfo {
        access_key_id: access_key,
        secret_access_key: secret_key,
        session_token: String::new(),
        expiration: String::new(),
    })
}

/// If `--target` is a `tag:Key=Val,...` spec, look up the first running EC2
/// instance matching ALL tags exactly and replace the spec with its
/// `i-...` instance id. Returns the command unchanged when no `tag:` is
/// found.
pub async fn resolve_tag_target_in_command(command: &str) -> Result<String, String> {
    let tag_re = Regex::new(r#"--target\s+"?(tag:[^"\s]+)"?"#).unwrap();
    let caps = match tag_re.captures(command) {
        Some(c) => c,
        None => return Ok(command.to_string()),
    };
    let full_match = caps[0].to_string();
    let tag_spec = caps[1].to_string();
    let tag_part = tag_spec.strip_prefix("tag:").unwrap_or(&tag_spec);
    let tag_pairs: Vec<(String, String)> = tag_part
        .split(',')
        .filter_map(|pair| {
            let mut it = pair.splitn(2, '=');
            Some((
                it.next()?.trim().to_string(),
                it.next()?.trim().to_string(),
            ))
        })
        .collect();
    if tag_pairs.is_empty() {
        return Err(format!("No valid tag key=value pairs in {tag_spec}"));
    }
    let tag_filters: String = tag_pairs
        .iter()
        .map(|(k, v)| format!("[?Tags[?Key=='{k}' && Value=='{v}']]"))
        .collect();
    let jmespath = format!("Reservations[].Instances{tag_filters} | [] | [0].InstanceId");
    let region = Regex::new(r"--region\s+(\S+)")
        .unwrap()
        .captures(command)
        .map(|c| c[1].to_string());
    let profile = Regex::new(r"--profile\s+(\S+)")
        .unwrap()
        .captures(command)
        .map(|c| c[1].to_string());

    let mut cmd = Command::new("aws");
    cmd.arg("ec2")
        .arg("describe-instances")
        .arg("--filters")
        .arg("Name=instance-state-name,Values=running")
        .arg("--query")
        .arg(&jmespath)
        .arg("--output")
        .arg("text")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(r) = &region {
        cmd.arg("--region").arg(r);
    }
    if let Some(p) = &profile {
        cmd.arg("--profile").arg(p);
    }
    let output = cmd
        .output()
        .await
        .map_err(|e| format!("EC2 lookup failed: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "EC2 describe-instances failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let id = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if id.is_empty() || id == "None" {
        return Err(format!("No running instance matched {tag_spec}"));
    }
    Ok(command.replace(&full_match, &format!("--target {id}")))
}

/// Wrap `--parameters {...}` in single quotes so the shell does not
/// interpret braces, brackets, or quotes.
pub fn quote_parameters_for_shell(command: &str) -> String {
    let re = Regex::new(r"--parameters\s+(\{.+\})").unwrap();
    if let Some(caps) = re.captures(command) {
        let json_val = &caps[1];
        command.replace(json_val, &format!("'{json_val}'"))
    } else {
        command.to_string()
    }
}
