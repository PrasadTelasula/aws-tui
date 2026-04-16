use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Alias {
    pub name: String,
    pub command: String,
    pub kind: AliasKind,
    pub group: Option<String>,
    pub subgroup: Option<String>,
    pub profile: Option<String>,
    pub region: Option<String>,
    pub target: Option<String>,
    pub sso_session_name: Option<String>,
    pub ssm_document: Option<String>,
    pub ssm_local_port: Option<String>,
    pub ssm_remote_port: Option<String>,
    pub ssm_host: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum AliasKind {
    SsoLogin,
    SsmSession,
    IamProfile,
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AliasesResponse {
    pub path: String,
    pub aliases: Vec<Alias>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum SessionState {
    Stopped,
    Starting,
    Running,
    Connected,
    Expired,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionStatus {
    pub alias: String,
    pub state: SessionState,
    pub pid: Option<u32>,
    pub started_at: Option<String>,
    pub error_message: Option<String>,
    pub sso_profile: Option<String>,
    pub identity_arn: Option<String>,
    pub identity_account: Option<String>,
    pub token_expires_at: Option<String>,
    pub token_remaining_secs: Option<u64>,
    pub has_credentials: bool,
}

impl SessionStatus {
    pub fn stopped(alias: &str) -> Self {
        Self {
            alias: alias.to_string(),
            state: SessionState::Stopped,
            pid: None,
            started_at: None,
            error_message: None,
            sso_profile: None,
            identity_arn: None,
            identity_account: None,
            token_expires_at: None,
            token_remaining_secs: None,
            has_credentials: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialInfo {
    pub access_key_id: String,
    pub secret_access_key: String,
    pub session_token: String,
    pub expiration: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Instance {
    pub id: String,
    pub name: Option<String>,
    pub state: String,
    pub instance_type: String,
    pub private_ip: Option<String>,
    pub public_ip: Option<String>,
    pub az: Option<String>,
    pub vpc_id: Option<String>,
    pub tags: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Cluster {
    pub name: String,
    pub arn: String,
    pub status: String,
    pub running_tasks: u32,
    pub services_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Service {
    pub name: String,
    pub arn: String,
    pub cluster: String,
    pub status: String,
    pub desired: u32,
    pub running: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    pub arn: String,
    pub cluster: String,
    pub service: Option<String>,
    pub last_status: String,
    pub desired_status: String,
    pub launch_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Container {
    pub name: String,
    pub task_arn: String,
    pub image: String,
    pub last_status: String,
    pub health: Option<String>,
}
