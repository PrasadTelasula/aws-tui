use crate::config::{self, AppConfig, AppState};
use crate::model::*;
use crate::parser;
use std::collections::HashMap;
use tauri::State;

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

#[tauri::command]
pub async fn list_instances(
    _profile: Option<String>,
    _region: Option<String>,
) -> Result<Vec<Instance>, String> {
    let mut tags = HashMap::new();
    tags.insert("Environment".into(), "production".into());
    tags.insert("Owner".into(), "platform".into());
    Ok(vec![
        Instance {
            id: "i-0abc123def456".into(),
            name: Some("web-prod-01".into()),
            state: "running".into(),
            instance_type: "t3.medium".into(),
            private_ip: Some("10.0.1.24".into()),
            public_ip: Some("54.12.34.56".into()),
            az: Some("us-east-1a".into()),
            vpc_id: Some("vpc-0abc".into()),
            tags: tags.clone(),
        },
        Instance {
            id: "i-0def789abc012".into(),
            name: Some("worker-prod-02".into()),
            state: "stopped".into(),
            instance_type: "t3.large".into(),
            private_ip: Some("10.0.1.25".into()),
            public_ip: None,
            az: Some("us-east-1b".into()),
            vpc_id: Some("vpc-0abc".into()),
            tags,
        },
    ])
}

#[tauri::command]
pub async fn describe_instance(id: String) -> Result<serde_json::Value, String> {
    Ok(serde_json::json!({
        "InstanceId": id,
        "InstanceType": "t3.medium",
        "State": { "Name": "running" },
        "Tags": []
    }))
}

#[tauri::command]
pub async fn list_clusters(
    _profile: Option<String>,
    _region: Option<String>,
) -> Result<Vec<Cluster>, String> {
    Ok(vec![Cluster {
        name: "prod-cluster".into(),
        arn: "arn:aws:ecs:us-east-1:123:cluster/prod-cluster".into(),
        status: "ACTIVE".into(),
        running_tasks: 12,
        services_count: 5,
    }])
}

#[tauri::command]
pub async fn list_services(cluster: String) -> Result<Vec<Service>, String> {
    Ok(vec![Service {
        name: "api".into(),
        arn: format!("arn:aws:ecs:us-east-1:123:service/{}/api", cluster),
        cluster: cluster.clone(),
        status: "ACTIVE".into(),
        desired: 3,
        running: 3,
    }])
}

#[tauri::command]
pub async fn list_tasks(
    cluster: String,
    service: Option<String>,
) -> Result<Vec<Task>, String> {
    Ok(vec![Task {
        arn: format!("arn:aws:ecs:us-east-1:123:task/{}/abc123", cluster),
        cluster,
        service,
        last_status: "RUNNING".into(),
        desired_status: "RUNNING".into(),
        launch_type: "FARGATE".into(),
    }])
}

#[tauri::command]
pub async fn list_containers(_task_arn: String) -> Result<Vec<Container>, String> {
    Ok(vec![Container {
        name: "app".into(),
        task_arn: "arn:aws:ecs:us-east-1:123:task/prod/abc123".into(),
        image: "123.dkr.ecr.us-east-1.amazonaws.com/app:latest".into(),
        last_status: "RUNNING".into(),
        health: Some("HEALTHY".into()),
    }])
}

#[tauri::command]
pub async fn complete_aws_cli(_line: String, _cursor: u32) -> Result<Vec<String>, String> {
    Ok(vec![])
}

#[tauri::command]
pub async fn aws_whoami(_profile: Option<String>) -> Result<serde_json::Value, String> {
    Ok(serde_json::json!({
        "UserId": "AIDAEXAMPLE",
        "Account": "123456789012",
        "Arn": "arn:aws:iam::123456789012:user/example"
    }))
}
