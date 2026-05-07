//! Parser for the user's AWS CLI config files (`~/.aws/config` and
//! `~/.aws/credentials`). Used by the Settings page and topbar profile
//! dropdown so the app can show the user every profile they've already
//! configured — not just the ones with active sessions.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AwsProfile {
    pub name: String,
    pub region: Option<String>,
    /// Linked SSO session block name (newer SSO config style).
    pub sso_session: Option<String>,
    pub sso_account_id: Option<String>,
    pub sso_role_name: Option<String>,
    /// Legacy SSO fields (single-profile SSO without an [sso-session …] block).
    pub sso_start_url: Option<String>,
    pub sso_region: Option<String>,
    /// "config" or "credentials" — where the profile was first discovered.
    pub source: String,
    /// True when the profile has any sso_* setting, OR a linked session that
    /// itself has sso_start_url set.
    pub is_sso: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AwsConfigSnapshot {
    pub config_path: Option<String>,
    pub credentials_path: Option<String>,
    pub profiles: Vec<AwsProfile>,
    pub sso_sessions: Vec<String>,
}

fn config_path() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("AWS_CONFIG_FILE") {
        return Some(PathBuf::from(p));
    }
    dirs::home_dir().map(|h| h.join(".aws").join("config"))
}

fn credentials_path() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("AWS_SHARED_CREDENTIALS_FILE") {
        return Some(PathBuf::from(p));
    }
    dirs::home_dir().map(|h| h.join(".aws").join("credentials"))
}

#[derive(Debug, Default)]
struct SsoSession {
    sso_start_url: Option<String>,
    sso_region: Option<String>,
}

/// Parse `~/.aws/config`. Returns:
///   - profiles keyed by their bare profile name (no `profile ` prefix)
///   - sso-session blocks keyed by their name
fn parse_config(content: &str) -> (Vec<AwsProfile>, BTreeMap<String, SsoSession>) {
    let mut profiles: Vec<AwsProfile> = Vec::new();
    let mut sso_sessions: BTreeMap<String, SsoSession> = BTreeMap::new();

    enum Section {
        Profile(usize),
        Sso(String),
        Skip,
    }
    let mut section: Section = Section::Skip;

    for raw in content.lines() {
        let trimmed = raw.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with(';') {
            continue;
        }
        if let Some(inner) = trimmed.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
            let inner = inner.trim();
            if inner == "default" {
                profiles.push(AwsProfile {
                    name: "default".into(),
                    source: "config".into(),
                    ..Default::default()
                });
                section = Section::Profile(profiles.len() - 1);
            } else if let Some(name) = inner.strip_prefix("profile ") {
                profiles.push(AwsProfile {
                    name: name.trim().into(),
                    source: "config".into(),
                    ..Default::default()
                });
                section = Section::Profile(profiles.len() - 1);
            } else if let Some(name) = inner.strip_prefix("sso-session ") {
                sso_sessions.entry(name.trim().into()).or_default();
                section = Section::Sso(name.trim().into());
            } else {
                section = Section::Skip;
            }
            continue;
        }
        let Some((key, value)) = trimmed.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim();
        match &section {
            Section::Profile(idx) => {
                let p = &mut profiles[*idx];
                match key {
                    "region" => p.region = Some(value.into()),
                    "sso_session" => p.sso_session = Some(value.into()),
                    "sso_account_id" => p.sso_account_id = Some(value.into()),
                    "sso_role_name" => p.sso_role_name = Some(value.into()),
                    "sso_start_url" => p.sso_start_url = Some(value.into()),
                    "sso_region" => p.sso_region = Some(value.into()),
                    _ => {}
                }
            }
            Section::Sso(name) => {
                let s = sso_sessions.entry(name.clone()).or_default();
                match key {
                    "sso_start_url" => s.sso_start_url = Some(value.into()),
                    "sso_region" => s.sso_region = Some(value.into()),
                    _ => {}
                }
            }
            Section::Skip => {}
        }
    }

    (profiles, sso_sessions)
}

/// Parse `~/.aws/credentials`. Each `[NAME]` is a profile.
fn parse_credentials(content: &str) -> Vec<AwsProfile> {
    let mut profiles: Vec<AwsProfile> = Vec::new();
    let mut current: Option<usize> = None;
    for raw in content.lines() {
        let trimmed = raw.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with(';') {
            continue;
        }
        if let Some(inner) = trimmed.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
            profiles.push(AwsProfile {
                name: inner.trim().into(),
                source: "credentials".into(),
                ..Default::default()
            });
            current = Some(profiles.len() - 1);
            continue;
        }
        if let Some((key, value)) = trimmed.split_once('=') {
            if let Some(idx) = current {
                if key.trim() == "region" {
                    profiles[idx].region = Some(value.trim().into());
                }
            }
        }
    }
    profiles
}

pub fn snapshot() -> AwsConfigSnapshot {
    let cfg = config_path();
    let creds = credentials_path();

    let (cfg_profiles, sessions) = cfg
        .as_ref()
        .and_then(|p| fs::read_to_string(p).ok())
        .map(|s| parse_config(&s))
        .unwrap_or_default();

    let cred_profiles = creds
        .as_ref()
        .and_then(|p| fs::read_to_string(p).ok())
        .map(|s| parse_credentials(&s))
        .unwrap_or_default();

    // Merge: config file wins; credentials file fills in profiles missing
    // from config (e.g. plain access-key-only profiles).
    let mut by_name: BTreeMap<String, AwsProfile> = BTreeMap::new();
    for p in cfg_profiles {
        by_name.insert(p.name.clone(), p);
    }
    for p in cred_profiles {
        by_name.entry(p.name.clone()).or_insert(p);
    }

    // Mark profiles as SSO-backed.
    for p in by_name.values_mut() {
        let inline_sso = p.sso_start_url.is_some();
        let session_sso = p
            .sso_session
            .as_ref()
            .and_then(|s| sessions.get(s))
            .map(|s| s.sso_start_url.is_some())
            .unwrap_or(false);
        p.is_sso = inline_sso || session_sso;
    }

    AwsConfigSnapshot {
        config_path: cfg.map(|p| p.to_string_lossy().into_owned()),
        credentials_path: creds.map(|p| p.to_string_lossy().into_owned()),
        profiles: by_name.into_values().collect(),
        sso_sessions: sessions.into_keys().collect(),
    }
}
