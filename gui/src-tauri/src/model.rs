use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Alias {
    pub name: String,
    pub command: String,
    pub kind: AliasKind,
    pub profile: Option<String>,
    pub region: Option<String>,
    pub target: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionStatus {
    pub alias: String,
    pub state: SessionState,
    pub pid: Option<u32>,
    pub started_at: Option<String>,
    pub expires_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SessionState {
    Idle,
    Starting,
    Active,
    Expired,
    Error,
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
    pub tags: std::collections::HashMap<String, String>,
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
