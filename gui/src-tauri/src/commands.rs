use crate::config::{self, AppConfig, AppState};
use crate::model::*;
use crate::parser;
use std::collections::HashMap;
use std::process::Stdio;
use tauri::State;
use tokio::process::Command;

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

async fn invoke_aws(
    args: Vec<String>,
    profile: &Option<String>,
    region: &Option<String>,
) -> Result<serde_json::Value, String> {
    let mut cmd = Command::new("aws");
    cmd.args(&args);
    if let Some(p) = profile {
        cmd.args(["--profile", p]);
    }
    if let Some(r) = region {
        cmd.args(["--region", r]);
    }
    cmd.args(["--output", "json"]);
    cmd.stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::null());

    let out = cmd.output().await.map_err(|e| format!("aws cli: {e}"))?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    serde_json::from_str(&String::from_utf8_lossy(&out.stdout)).map_err(|e| e.to_string())
}

fn a(s: &str) -> String {
    s.to_string()
}

// ---------------------------------------------------------------------------
// Aliases / config
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn list_aliases(
    path: Option<String>,
    state: State<'_, AppState>,
) -> Result<AliasesResponse, String> {
    let resolved = if path.is_some() {
        path.clone()
    } else {
        state.config.lock().unwrap().aliases_path.clone()
    };

    let (loaded_path, aliases) = parser::read_aliases(resolved.as_deref())?;

    if path.is_some() {
        let mut cfg = state.config.lock().unwrap();
        cfg.aliases_path = Some(loaded_path.to_string_lossy().into_owned());
        let snapshot = cfg.clone();
        drop(cfg);
        let _ = config::save(&snapshot);
    }

    Ok(AliasesResponse {
        path: loaded_path.to_string_lossy().into_owned(),
        aliases,
    })
}

#[tauri::command]
pub async fn get_config(state: State<'_, AppState>) -> Result<AppConfig, String> {
    Ok(state.config.lock().unwrap().clone())
}

/// Read profiles from the user's `~/.aws/config` and `~/.aws/credentials`.
/// Returns each profile with the most useful fields surfaced (region,
/// linked SSO session, role/account, source file).
#[tauri::command]
pub async fn list_aws_profiles() -> Result<crate::aws_config::AwsConfigSnapshot, String> {
    Ok(crate::aws_config::snapshot())
}

#[tauri::command]
pub async fn set_aliases_path(
    path: String,
    state: State<'_, AppState>,
) -> Result<AliasesResponse, String> {
    let (loaded_path, aliases) = parser::read_aliases(Some(&path))?;
    {
        let mut cfg = state.config.lock().unwrap();
        cfg.aliases_path = Some(loaded_path.to_string_lossy().into_owned());
        let snapshot = cfg.clone();
        drop(cfg);
        config::save(&snapshot)?;
    }
    Ok(AliasesResponse {
        path: loaded_path.to_string_lossy().into_owned(),
        aliases,
    })
}

/// Persist a list of aliases to disk in the shell-alias format the parser
/// understands. If `path` is None, falls back to the currently-loaded path,
/// or to ~/.aws_tui_config when nothing is loaded yet (creating the file).
#[tauri::command]
pub async fn save_aliases(
    path: Option<String>,
    aliases: Vec<Alias>,
    state: State<'_, AppState>,
) -> Result<AliasesResponse, String> {
    use std::path::PathBuf;

    let resolved: PathBuf = match path
        .or_else(|| state.config.lock().unwrap().aliases_path.clone())
    {
        Some(p) => PathBuf::from(p),
        None => dirs::home_dir()
            .ok_or_else(|| "no home directory".to_string())?
            .join(".aws_tui_config"),
    };

    if let Some(parent) = resolved.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("create parent dir: {e}"))?;
    }

    let content = parser::serialize(&aliases);
    std::fs::write(&resolved, content)
        .map_err(|e| format!("write {}: {}", resolved.display(), e))?;

    {
        let mut cfg = state.config.lock().unwrap();
        cfg.aliases_path = Some(resolved.to_string_lossy().into_owned());
        let snapshot = cfg.clone();
        drop(cfg);
        config::save(&snapshot)?;
    }

    // Re-read so the response reflects exactly what's on disk now.
    let reloaded = parser::read_aliases_at(&resolved)?;
    Ok(AliasesResponse {
        path: resolved.to_string_lossy().into_owned(),
        aliases: reloaded,
    })
}

// ---------------------------------------------------------------------------
// EC2
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn list_instances(
    profile: Option<String>,
    region: Option<String>,
) -> Result<Vec<Instance>, String> {
    let json = invoke_aws(
        vec![
            a("ec2"),
            a("describe-instances"),
            a("--filters"),
            a("Name=instance-state-name,Values=running,stopped,stopping,pending"),
        ],
        &profile,
        &region,
    )
    .await?;

    let mut instances = Vec::new();
    if let Some(reservations) = json.get("Reservations").and_then(|v| v.as_array()) {
        for res in reservations {
            if let Some(insts) = res.get("Instances").and_then(|v| v.as_array()) {
                for inst in insts {
                    let tags: HashMap<String, String> = inst
                        .get("Tags")
                        .and_then(|v| v.as_array())
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|t| {
                                    let k = t.get("Key")?.as_str()?.to_string();
                                    let v = t.get("Value")?.as_str()?.to_string();
                                    Some((k, v))
                                })
                                .collect()
                        })
                        .unwrap_or_default();

                    let name = tags.get("Name").cloned();

                    instances.push(Instance {
                        id: inst
                            .get("InstanceId")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                        name,
                        state: inst
                            .get("State")
                            .and_then(|s| s.get("Name"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("unknown")
                            .to_string(),
                        instance_type: inst
                            .get("InstanceType")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                        private_ip: inst
                            .get("PrivateIpAddress")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string()),
                        public_ip: inst
                            .get("PublicIpAddress")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string()),
                        az: inst
                            .get("Placement")
                            .and_then(|p| p.get("AvailabilityZone"))
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string()),
                        vpc_id: inst
                            .get("VpcId")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string()),
                        tags,
                    });
                }
            }
        }
    }

    Ok(instances)
}

#[tauri::command]
pub async fn describe_instance(
    id: String,
    profile: Option<String>,
    region: Option<String>,
) -> Result<serde_json::Value, String> {
    let json = invoke_aws(
        vec![
            a("ec2"),
            a("describe-instances"),
            a("--instance-ids"),
            id,
        ],
        &profile,
        &region,
    )
    .await?;

    // Return the first instance object for convenience
    let instance = json
        .get("Reservations")
        .and_then(|v| v.as_array())
        .and_then(|a| a.first())
        .and_then(|r| r.get("Instances"))
        .and_then(|v| v.as_array())
        .and_then(|a| a.first())
        .cloned()
        .unwrap_or(json);
    Ok(instance)
}

// ---------------------------------------------------------------------------
// ECS clusters
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn list_clusters(
    profile: Option<String>,
    region: Option<String>,
) -> Result<Vec<Cluster>, String> {
    // Step 1: collect all cluster ARNs (paginate)
    let mut all_arns: Vec<String> = Vec::new();
    let mut next_token: Option<String> = None;
    loop {
        let mut args = vec![a("ecs"), a("list-clusters"), a("--max-results"), a("100")];
        if let Some(ref tok) = next_token {
            args.push(a("--next-token"));
            args.push(tok.clone());
        }
        let page = invoke_aws(args, &profile, &region).await?;
        if let Some(arns) = page.get("clusterArns").and_then(|v| v.as_array()) {
            all_arns.extend(arns.iter().filter_map(|v| v.as_str().map(|s| s.to_string())));
        }
        next_token = page
            .get("nextToken")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        if next_token.is_none() {
            break;
        }
    }

    if all_arns.is_empty() {
        return Ok(vec![]);
    }

    // Step 2: describe all clusters in one call (limit 100 per call)
    let mut clusters = Vec::new();
    for chunk in all_arns.chunks(100) {
        let mut args = vec![a("ecs"), a("describe-clusters"), a("--clusters")];
        args.extend(chunk.iter().cloned());
        let json = invoke_aws(args, &profile, &region).await?;
        if let Some(arr) = json.get("clusters").and_then(|v| v.as_array()) {
            for c in arr {
                clusters.push(Cluster {
                    name: c
                        .get("clusterName")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    arn: c
                        .get("clusterArn")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    status: c
                        .get("status")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    running_tasks: c
                        .get("runningTasksCount")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0) as u32,
                    services_count: c
                        .get("activeServicesCount")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0) as u32,
                });
            }
        }
    }

    Ok(clusters)
}

// ---------------------------------------------------------------------------
// ECS services
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn list_services(
    cluster: String,
    profile: Option<String>,
    region: Option<String>,
) -> Result<Vec<Service>, String> {
    // Step 1: list service ARNs (paginate, max 100 per page)
    let mut all_arns: Vec<String> = Vec::new();
    let mut next_token: Option<String> = None;
    loop {
        let mut args = vec![
            a("ecs"),
            a("list-services"),
            a("--cluster"),
            cluster.clone(),
            a("--max-results"),
            a("100"),
        ];
        if let Some(ref tok) = next_token {
            args.push(a("--next-token"));
            args.push(tok.clone());
        }
        let page = invoke_aws(args, &profile, &region).await?;
        if let Some(arns) = page.get("serviceArns").and_then(|v| v.as_array()) {
            all_arns.extend(arns.iter().filter_map(|v| v.as_str().map(|s| s.to_string())));
        }
        next_token = page
            .get("nextToken")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        if next_token.is_none() {
            break;
        }
    }

    if all_arns.is_empty() {
        return Ok(vec![]);
    }

    // Step 2: describe services in batches of 10
    let mut services = Vec::new();
    for chunk in all_arns.chunks(10) {
        let mut args = vec![
            a("ecs"),
            a("describe-services"),
            a("--cluster"),
            cluster.clone(),
            a("--services"),
        ];
        args.extend(chunk.iter().cloned());
        let json = invoke_aws(args, &profile, &region).await?;
        if let Some(arr) = json.get("services").and_then(|v| v.as_array()) {
            for s in arr {
                let arn = s
                    .get("serviceArn")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let name = s
                    .get("serviceName")
                    .and_then(|v| v.as_str())
                    .unwrap_or_else(|| arn.split('/').last().unwrap_or(""))
                    .to_string();
                services.push(Service {
                    name,
                    arn,
                    cluster: cluster.clone(),
                    status: s
                        .get("status")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    desired: s
                        .get("desiredCount")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0) as u32,
                    running: s
                        .get("runningCount")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0) as u32,
                });
            }
        }
    }

    Ok(services)
}

// ---------------------------------------------------------------------------
// ECS tasks
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn list_tasks(
    cluster: String,
    service: Option<String>,
    profile: Option<String>,
    region: Option<String>,
) -> Result<Vec<Task>, String> {
    // Step 1: list task ARNs (paginate)
    let mut all_arns: Vec<String> = Vec::new();
    let mut next_token: Option<String> = None;
    loop {
        let mut args = vec![
            a("ecs"),
            a("list-tasks"),
            a("--cluster"),
            cluster.clone(),
            a("--max-results"),
            a("100"),
        ];
        if let Some(ref svc) = service {
            args.push(a("--service-name"));
            args.push(svc.clone());
        }
        if let Some(ref tok) = next_token {
            args.push(a("--next-token"));
            args.push(tok.clone());
        }
        let page = invoke_aws(args, &profile, &region).await?;
        if let Some(arns) = page.get("taskArns").and_then(|v| v.as_array()) {
            all_arns.extend(arns.iter().filter_map(|v| v.as_str().map(|s| s.to_string())));
        }
        next_token = page
            .get("nextToken")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        if next_token.is_none() {
            break;
        }
    }

    if all_arns.is_empty() {
        return Ok(vec![]);
    }

    // Step 2: describe tasks in batches of 100
    let mut tasks = Vec::new();
    for chunk in all_arns.chunks(100) {
        let mut args = vec![
            a("ecs"),
            a("describe-tasks"),
            a("--cluster"),
            cluster.clone(),
            a("--tasks"),
        ];
        args.extend(chunk.iter().cloned());
        let json = invoke_aws(args, &profile, &region).await?;
        if let Some(arr) = json.get("tasks").and_then(|v| v.as_array()) {
            for t in arr {
                // Parse service name from the group field ("service:<name>")
                let svc = t
                    .get("group")
                    .and_then(|v| v.as_str())
                    .and_then(|g| g.strip_prefix("service:"))
                    .map(|s| s.to_string());

                tasks.push(Task {
                    arn: t
                        .get("taskArn")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    cluster: cluster.clone(),
                    service: svc,
                    last_status: t
                        .get("lastStatus")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    desired_status: t
                        .get("desiredStatus")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    launch_type: t
                        .get("launchType")
                        .and_then(|v| v.as_str())
                        .unwrap_or("EC2")
                        .to_string(),
                });
            }
        }
    }

    Ok(tasks)
}

// ---------------------------------------------------------------------------
// ECS containers
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn list_containers(
    task_arn: String,
    cluster: String,
    profile: Option<String>,
    region: Option<String>,
) -> Result<Vec<Container>, String> {
    let json = invoke_aws(
        vec![
            a("ecs"),
            a("describe-tasks"),
            a("--cluster"),
            cluster,
            a("--tasks"),
            task_arn.clone(),
        ],
        &profile,
        &region,
    )
    .await?;

    let mut containers = Vec::new();
    if let Some(tasks) = json.get("tasks").and_then(|v| v.as_array()) {
        if let Some(task) = tasks.first() {
            if let Some(conts) = task.get("containers").and_then(|v| v.as_array()) {
                for c in conts {
                    containers.push(Container {
                        name: c
                            .get("name")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                        task_arn: task_arn.clone(),
                        image: c
                            .get("image")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                        last_status: c
                            .get("lastStatus")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                        health: c
                            .get("healthStatus")
                            .and_then(|v| v.as_str())
                            .filter(|&s| s != "UNKNOWN")
                            .map(|s| s.to_string()),
                    });
                }
            }
        }
    }

    Ok(containers)
}

// ---------------------------------------------------------------------------
// AWS CLI completion
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn complete_aws_cli(line: String, cursor: u32) -> Result<Vec<String>, String> {
    let result = Command::new("aws_completer")
        .env("COMP_LINE", &line)
        .env("COMP_POINT", cursor.to_string())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .stdin(Stdio::null())
        .output()
        .await;

    match result {
        Ok(out) if out.status.success() => {
            let text = String::from_utf8_lossy(&out.stdout);
            Ok(text
                .lines()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect())
        }
        _ => Ok(vec![]),
    }
}

// ---------------------------------------------------------------------------
// STS identity
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn aws_whoami(profile: Option<String>) -> Result<serde_json::Value, String> {
    let json = invoke_aws(
        vec![a("sts"), a("get-caller-identity")],
        &profile,
        &None,
    )
    .await?;
    Ok(json)
}
