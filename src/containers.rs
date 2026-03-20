use crate::instances::REGIONS;
use std::process::Stdio;
use tokio::process::Command;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};

// ─── Sub-tab & Focus ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum ContainersSubTab {
    Ecs,
    Eks,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ContainersFocus {
    RegionList,
    SubTabBar,
    ClusterList,
    DetailList,
}

// ─── ECS tree types ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum EcsTreeItemKind {
    Cluster,
    Service,
    Task,
    Container,
}

#[derive(Debug, Clone)]
pub struct EcsTreeItem {
    pub kind: EcsTreeItemKind,
    pub depth: u8,
    pub name: String,
    pub status: String,
    pub expanded: bool,
    pub loading: bool,
    pub cluster_name: String,  // set for Service, Task, Container
    pub service_name: String,  // set for Task, Container
    pub launch_type: String,   // set for Task (FARGATE/EC2)
    pub extra: String,         // misc display info
    pub count_a: i64,          // running for service, running_tasks for cluster
    pub count_b: i64,          // desired for service, pending_tasks for cluster
}

// ─── ECS data types (used during fetch) ──────────────────────────────────────

#[derive(Debug, Clone)]
pub struct EcsCluster {
    pub name: String,
    pub status: String,
    pub active_services: i64,
    pub running_tasks: i64,
    pub pending_tasks: i64,
}

#[derive(Debug, Clone)]
pub struct EcsService {
    pub name: String,
    pub status: String,
    pub desired: i64,
    pub running: i64,
    pub task_definition: String,
}

#[derive(Debug, Clone)]
pub struct EcsTaskData {
    pub task_id: String,
    pub status: String,
    pub launch_type: String,
    pub task_definition: String,
    pub containers: Vec<EcsContainerData>,
}

#[derive(Debug, Clone)]
pub struct EcsContainerData {
    pub name: String,
    pub image: String,
    pub status: String,
}

// ─── EKS data types ───────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct EksCluster {
    pub name: String,
    pub status: String,
    pub version: String,
}

#[derive(Debug, Clone)]
pub struct EksNodegroup {
    pub name: String,
    pub status: String,
    pub desired: i64,
    pub min: i64,
    pub max: i64,
    pub instance_types: String,
}

// ─── Async result channel ─────────────────────────────────────────────────────

pub enum FetchResult {
    EcsClusters(Vec<EcsCluster>),
    EcsServices(String, Vec<EcsService>),           // cluster_name, services
    EcsTasks(String, String, Vec<EcsTaskData>),     // cluster_name, service_name, tasks
    EksClusters(Vec<EksCluster>),
    EksNodegroups(String, Vec<EksNodegroup>),        // cluster_name, nodegroups
    Error(String),
}

// ─── State ───────────────────────────────────────────────────────────────────

pub struct ContainersState {
    // Profile / region (synced from Sessions tab)
    pub profiles: Vec<String>,
    pub active_profile_idx: usize,
    pub region_idx: usize,
    pub regions: Vec<String>,
    pub region_dropdown_open: bool,

    // Sub-tab & focus
    pub sub_tab: ContainersSubTab,
    pub focus: ContainersFocus,

    // ECS - tree-based
    pub ecs_tree: Vec<EcsTreeItem>,
    pub selected_ecs_tree: usize,
    pub loading_ecs_clusters: bool,

    // EKS
    pub eks_clusters: Vec<EksCluster>,
    pub selected_eks_cluster: usize,
    pub loading_eks_clusters: bool,
    pub eks_nodegroups: Vec<EksNodegroup>,
    pub selected_eks_nodegroup: usize,
    pub loading_eks_nodegroups: bool,
    pub eks_nodegroups_for: String,

    pub last_error: Option<String>,

    fetch_tx: UnboundedSender<FetchResult>,
    pub fetch_rx: UnboundedReceiver<FetchResult>,
}

impl ContainersState {
    pub fn new() -> Self {
        let (fetch_tx, fetch_rx) = mpsc::unbounded_channel();
        Self {
            profiles: Vec::new(),
            active_profile_idx: 0,
            region_idx: 3, // us-west-2 default
            regions: REGIONS.iter().map(|s| s.to_string()).collect(),
            region_dropdown_open: false,
            sub_tab: ContainersSubTab::Ecs,
            focus: ContainersFocus::RegionList,
            ecs_tree: Vec::new(),
            selected_ecs_tree: 0,
            loading_ecs_clusters: false,
            eks_clusters: Vec::new(),
            selected_eks_cluster: 0,
            loading_eks_clusters: false,
            eks_nodegroups: Vec::new(),
            selected_eks_nodegroup: 0,
            loading_eks_nodegroups: false,
            eks_nodegroups_for: String::new(),
            last_error: None,
            fetch_tx,
            fetch_rx,
        }
    }

    pub fn active_profile(&self) -> Option<&str> {
        self.profiles.get(self.active_profile_idx).map(|s| s.as_str())
    }

    pub fn active_region(&self) -> &str {
        &self.regions[self.region_idx]
    }

    // ── Profile / region navigation ──────────────────────────────────

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
        self.region_idx = if self.region_idx == 0 {
            self.regions.len() - 1
        } else {
            self.region_idx - 1
        };
    }

    // ── ECS tree navigation ───────────────────────────────────────────

    pub fn ecs_next_item(&mut self) {
        if !self.ecs_tree.is_empty() {
            self.selected_ecs_tree = (self.selected_ecs_tree + 1) % self.ecs_tree.len();
        }
    }

    pub fn ecs_prev_item(&mut self) {
        if !self.ecs_tree.is_empty() {
            self.selected_ecs_tree = if self.selected_ecs_tree == 0 {
                self.ecs_tree.len() - 1
            } else {
                self.selected_ecs_tree - 1
            };
        }
    }

    /// Toggle expand/collapse of the selected ECS tree item.
    pub fn ecs_toggle_selected(&mut self) {
        if self.ecs_tree.is_empty() { return; }
        let idx = self.selected_ecs_tree;
        if self.ecs_tree[idx].expanded {
            self.ecs_collapse_at(idx);
        } else {
            match self.ecs_tree[idx].kind {
                EcsTreeItemKind::Cluster => {
                    let name = self.ecs_tree[idx].name.clone();
                    self.ecs_tree[idx].loading = true;
                    self.fetch_ecs_services_for(name);
                }
                EcsTreeItemKind::Service => {
                    let cluster = self.ecs_tree[idx].cluster_name.clone();
                    let service = self.ecs_tree[idx].name.clone();
                    self.ecs_tree[idx].loading = true;
                    self.fetch_ecs_tasks_for(cluster, service);
                }
                EcsTreeItemKind::Task | EcsTreeItemKind::Container => {}
            }
        }
    }

    /// Collapse selected item (or move to parent if already collapsed).
    pub fn ecs_collapse_selected(&mut self) {
        if self.ecs_tree.is_empty() { return; }
        let idx = self.selected_ecs_tree;
        if self.ecs_tree[idx].expanded {
            self.ecs_collapse_at(idx);
        } else {
            // Move cursor to parent
            let depth = self.ecs_tree[idx].depth;
            if depth == 0 { return; }
            let mut i = idx;
            while i > 0 {
                i -= 1;
                if self.ecs_tree[i].depth < depth {
                    self.selected_ecs_tree = i;
                    return;
                }
            }
        }
    }

    fn ecs_collapse_at(&mut self, idx: usize) {
        let depth = self.ecs_tree[idx].depth;
        let start = idx + 1;
        let mut end = start;
        while end < self.ecs_tree.len() && self.ecs_tree[end].depth > depth {
            end += 1;
        }
        self.ecs_tree.drain(start..end);
        self.ecs_tree[idx].expanded = false;
        if self.selected_ecs_tree >= self.ecs_tree.len() {
            self.selected_ecs_tree = self.ecs_tree.len().saturating_sub(1);
        }
    }

    fn ecs_insert_children(&mut self, parent_idx: usize, children: Vec<EcsTreeItem>) {
        let parent_depth = self.ecs_tree[parent_idx].depth;
        let start = parent_idx + 1;
        let mut end = start;
        while end < self.ecs_tree.len() && self.ecs_tree[end].depth > parent_depth {
            end += 1;
        }
        self.ecs_tree.drain(start..end);
        for (i, item) in children.into_iter().enumerate() {
            self.ecs_tree.insert(start + i, item);
        }
        if self.selected_ecs_tree >= self.ecs_tree.len() {
            self.selected_ecs_tree = self.ecs_tree.len().saturating_sub(1);
        }
    }

    // ── EKS list navigation ───────────────────────────────────────────

    pub fn next_cluster(&mut self) {
        match self.sub_tab {
            ContainersSubTab::Ecs => self.ecs_next_item(),
            ContainersSubTab::Eks => {
                let len = self.eks_clusters.len();
                if len == 0 { return; }
                self.selected_eks_cluster = (self.selected_eks_cluster + 1) % len;
            }
        }
    }

    pub fn prev_cluster(&mut self) {
        match self.sub_tab {
            ContainersSubTab::Ecs => self.ecs_prev_item(),
            ContainersSubTab::Eks => {
                let len = self.eks_clusters.len();
                if len == 0 { return; }
                self.selected_eks_cluster = if self.selected_eks_cluster == 0 {
                    len - 1
                } else {
                    self.selected_eks_cluster - 1
                };
            }
        }
    }

    pub fn next_detail(&mut self) {
        match self.sub_tab {
            ContainersSubTab::Ecs => {}
            ContainersSubTab::Eks => {
                if !self.eks_nodegroups.is_empty() {
                    self.selected_eks_nodegroup =
                        (self.selected_eks_nodegroup + 1) % self.eks_nodegroups.len();
                }
            }
        }
    }

    pub fn prev_detail(&mut self) {
        match self.sub_tab {
            ContainersSubTab::Ecs => {}
            ContainersSubTab::Eks => {
                if !self.eks_nodegroups.is_empty() {
                    self.selected_eks_nodegroup = if self.selected_eks_nodegroup == 0 {
                        self.eks_nodegroups.len() - 1
                    } else {
                        self.selected_eks_nodegroup - 1
                    };
                }
            }
        }
    }

    pub fn cycle_focus(&mut self) {
        self.focus = match self.focus {
            ContainersFocus::RegionList  => ContainersFocus::SubTabBar,
            ContainersFocus::SubTabBar   => ContainersFocus::ClusterList,
            ContainersFocus::ClusterList => match self.sub_tab {
                ContainersSubTab::Ecs => ContainersFocus::RegionList,
                ContainersSubTab::Eks => ContainersFocus::DetailList,
            },
            ContainersFocus::DetailList  => ContainersFocus::RegionList,
        };
    }

    pub fn switch_sub_tab(&mut self) {
        self.sub_tab = match self.sub_tab {
            ContainersSubTab::Ecs => ContainersSubTab::Eks,
            ContainersSubTab::Eks => ContainersSubTab::Ecs,
        };
        // If we were on DetailList (EKS), snap back to ClusterList for ECS
        if self.sub_tab == ContainersSubTab::Ecs
            && self.focus == ContainersFocus::DetailList
        {
            self.focus = ContainersFocus::ClusterList;
        }
    }

    // ── Tick — drain async results ────────────────────────────────────

    pub fn tick(&mut self) {
        while let Ok(result) = self.fetch_rx.try_recv() {
            match result {
                FetchResult::EcsClusters(clusters) => {
                    self.loading_ecs_clusters = false;
                    self.last_error = None;
                    self.ecs_tree = clusters
                        .iter()
                        .map(|c| EcsTreeItem {
                            kind: EcsTreeItemKind::Cluster,
                            depth: 0,
                            name: c.name.clone(),
                            status: c.status.clone(),
                            expanded: false,
                            loading: false,
                            cluster_name: String::new(),
                            service_name: String::new(),
                            launch_type: String::new(),
                            extra: format!(
                                "{} svc  {} run",
                                c.active_services, c.running_tasks
                            ),
                            count_a: c.running_tasks,
                            count_b: c.pending_tasks,
                        })
                        .collect();
                    self.selected_ecs_tree = 0;
                }

                FetchResult::EcsServices(cluster_name, services) => {
                    self.last_error = None;
                    if let Some(cluster_idx) = self.ecs_tree.iter().position(|item| {
                        item.kind == EcsTreeItemKind::Cluster && item.name == cluster_name
                    }) {
                        self.ecs_tree[cluster_idx].loading = false;
                        self.ecs_tree[cluster_idx].expanded = true;
                        let children: Vec<EcsTreeItem> = services
                            .iter()
                            .map(|s| EcsTreeItem {
                                kind: EcsTreeItemKind::Service,
                                depth: 1,
                                name: s.name.clone(),
                                status: s.status.clone(),
                                expanded: false,
                                loading: false,
                                cluster_name: cluster_name.clone(),
                                service_name: String::new(),
                                launch_type: String::new(),
                                extra: s.task_definition.clone(),
                                count_a: s.running,
                                count_b: s.desired,
                            })
                            .collect();
                        self.ecs_insert_children(cluster_idx, children);
                    }
                }

                FetchResult::EcsTasks(cluster_name, service_name, tasks) => {
                    self.last_error = None;
                    if let Some(svc_idx) = self.ecs_tree.iter().position(|item| {
                        item.kind == EcsTreeItemKind::Service
                            && item.name == service_name
                            && item.cluster_name == cluster_name
                    }) {
                        self.ecs_tree[svc_idx].loading = false;
                        self.ecs_tree[svc_idx].expanded = true;
                        let mut children: Vec<EcsTreeItem> = Vec::new();
                        for task in &tasks {
                            children.push(EcsTreeItem {
                                kind: EcsTreeItemKind::Task,
                                depth: 2,
                                name: task.task_id.clone(),
                                status: task.status.clone(),
                                expanded: false,
                                loading: false,
                                cluster_name: cluster_name.clone(),
                                service_name: service_name.clone(),
                                launch_type: task.launch_type.clone(),
                                extra: task.task_definition.clone(),
                                count_a: 0,
                                count_b: 0,
                            });
                            for container in &task.containers {
                                children.push(EcsTreeItem {
                                    kind: EcsTreeItemKind::Container,
                                    depth: 3,
                                    name: container.name.clone(),
                                    status: container.status.clone(),
                                    expanded: false,
                                    loading: false,
                                    cluster_name: cluster_name.clone(),
                                    service_name: service_name.clone(),
                                    launch_type: String::new(),
                                    extra: container.image.clone(),
                                    count_a: 0,
                                    count_b: 0,
                                });
                            }
                        }
                        self.ecs_insert_children(svc_idx, children);
                    }
                }

                FetchResult::EksClusters(clusters) => {
                    self.eks_clusters = clusters;
                    self.selected_eks_cluster = 0;
                    self.loading_eks_clusters = false;
                    self.eks_nodegroups.clear();
                    self.eks_nodegroups_for.clear();
                    self.last_error = None;
                }
                FetchResult::EksNodegroups(cluster, nodegroups) => {
                    self.eks_nodegroups = nodegroups;
                    self.selected_eks_nodegroup = 0;
                    self.loading_eks_nodegroups = false;
                    self.eks_nodegroups_for = cluster;
                    self.last_error = None;
                }
                FetchResult::Error(e) => {
                    self.loading_ecs_clusters = false;
                    self.loading_eks_clusters = false;
                    self.loading_eks_nodegroups = false;
                    // Clear loading flag on any loading tree item
                    for item in &mut self.ecs_tree {
                        item.loading = false;
                    }
                    self.last_error = Some(e);
                }
            }
        }
    }

    // ── ECS fetching ──────────────────────────────────────────────────

    pub fn fetch_ecs_clusters(&mut self) {
        let profile = match self.active_profile() {
            Some(p) => p.to_string(),
            None => return,
        };
        let region = self.active_region().to_string();
        let tx = self.fetch_tx.clone();

        self.loading_ecs_clusters = true;
        self.ecs_tree.clear();
        self.selected_ecs_tree = 0;

        tokio::spawn(async move {
            let list_out = Command::new("aws")
                .args(["ecs", "list-clusters",
                       "--region", &region, "--profile", &profile,
                       "--output", "json"])
                .stdout(Stdio::piped()).stderr(Stdio::piped()).stdin(Stdio::null())
                .output().await;

            let arns: Vec<String> = match list_out {
                Ok(o) if o.status.success() => {
                    let v: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap_or_default();
                    v["clusterArns"].as_array().unwrap_or(&vec![])
                        .iter().filter_map(|a| a.as_str().map(String::from)).collect()
                }
                Ok(o) => {
                    let _ = tx.send(FetchResult::Error(
                        String::from_utf8_lossy(&o.stderr).trim().to_string()
                    ));
                    return;
                }
                Err(e) => { let _ = tx.send(FetchResult::Error(e.to_string())); return; }
            };

            if arns.is_empty() {
                let _ = tx.send(FetchResult::EcsClusters(vec![]));
                return;
            }

            let mut cmd = Command::new("aws");
            cmd.args(["ecs", "describe-clusters",
                      "--region", &region, "--profile", &profile,
                      "--output", "json", "--clusters"]);
            for arn in &arns { cmd.arg(arn); }
            cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).stdin(Stdio::null());

            match cmd.output().await {
                Ok(o) if o.status.success() => {
                    let v: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap_or_default();
                    let clusters = v["clusters"].as_array().unwrap_or(&vec![])
                        .iter()
                        .map(|c| EcsCluster {
                            name:            c["clusterName"].as_str().unwrap_or("unknown").to_string(),
                            status:          c["status"].as_str().unwrap_or("UNKNOWN").to_string(),
                            active_services: c["activeServicesCount"].as_i64().unwrap_or(0),
                            running_tasks:   c["runningTasksCount"].as_i64().unwrap_or(0),
                            pending_tasks:   c["pendingTasksCount"].as_i64().unwrap_or(0),
                        })
                        .collect();
                    let _ = tx.send(FetchResult::EcsClusters(clusters));
                }
                Ok(o) => { let _ = tx.send(FetchResult::Error(String::from_utf8_lossy(&o.stderr).trim().to_string())); }
                Err(e) => { let _ = tx.send(FetchResult::Error(e.to_string())); }
            }
        });
    }

    fn fetch_ecs_services_for(&mut self, cluster_name: String) {
        let profile = match self.active_profile() {
            Some(p) => p.to_string(),
            None => return,
        };
        let region = self.active_region().to_string();
        let tx = self.fetch_tx.clone();

        tokio::spawn(async move {
            let list_out = Command::new("aws")
                .args(["ecs", "list-services",
                       "--cluster", &cluster_name,
                       "--region", &region, "--profile", &profile,
                       "--output", "json"])
                .stdout(Stdio::piped()).stderr(Stdio::piped()).stdin(Stdio::null())
                .output().await;

            let arns: Vec<String> = match list_out {
                Ok(o) if o.status.success() => {
                    let v: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap_or_default();
                    v["serviceArns"].as_array().unwrap_or(&vec![])
                        .iter().filter_map(|a| a.as_str().map(String::from)).collect()
                }
                Ok(o) => {
                    let _ = tx.send(FetchResult::Error(String::from_utf8_lossy(&o.stderr).trim().to_string()));
                    return;
                }
                Err(e) => { let _ = tx.send(FetchResult::Error(e.to_string())); return; }
            };

            if arns.is_empty() {
                let _ = tx.send(FetchResult::EcsServices(cluster_name, vec![]));
                return;
            }

            let mut all_services: Vec<EcsService> = Vec::new();
            for chunk in arns.chunks(10) {
                let mut cmd = Command::new("aws");
                cmd.args(["ecs", "describe-services",
                          "--cluster", &cluster_name,
                          "--region", &region, "--profile", &profile,
                          "--output", "json", "--services"]);
                for arn in chunk { cmd.arg(arn); }
                cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).stdin(Stdio::null());

                if let Ok(o) = cmd.output().await {
                    if o.status.success() {
                        let v: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap_or_default();
                        if let Some(svcs) = v["services"].as_array() {
                            for s in svcs {
                                let td_full = s["taskDefinition"].as_str().unwrap_or("");
                                let td = td_full.split('/').last().unwrap_or(td_full).to_string();
                                all_services.push(EcsService {
                                    name:            s["serviceName"].as_str().unwrap_or("unknown").to_string(),
                                    status:          s["status"].as_str().unwrap_or("UNKNOWN").to_string(),
                                    desired:         s["desiredCount"].as_i64().unwrap_or(0),
                                    running:         s["runningCount"].as_i64().unwrap_or(0),
                                    task_definition: td,
                                });
                            }
                        }
                    }
                }
            }
            let _ = tx.send(FetchResult::EcsServices(cluster_name, all_services));
        });
    }

    fn fetch_ecs_tasks_for(&mut self, cluster_name: String, service_name: String) {
        let profile = match self.active_profile() {
            Some(p) => p.to_string(),
            None => return,
        };
        let region = self.active_region().to_string();
        let tx = self.fetch_tx.clone();

        tokio::spawn(async move {
            let list_out = Command::new("aws")
                .args(["ecs", "list-tasks",
                       "--cluster", &cluster_name,
                       "--service-name", &service_name,
                       "--region", &region, "--profile", &profile,
                       "--output", "json"])
                .stdout(Stdio::piped()).stderr(Stdio::piped()).stdin(Stdio::null())
                .output().await;

            let arns: Vec<String> = match list_out {
                Ok(o) if o.status.success() => {
                    let v: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap_or_default();
                    v["taskArns"].as_array().unwrap_or(&vec![])
                        .iter().filter_map(|a| a.as_str().map(String::from)).collect()
                }
                Ok(o) => {
                    let _ = tx.send(FetchResult::Error(String::from_utf8_lossy(&o.stderr).trim().to_string()));
                    return;
                }
                Err(e) => { let _ = tx.send(FetchResult::Error(e.to_string())); return; }
            };

            if arns.is_empty() {
                let _ = tx.send(FetchResult::EcsTasks(cluster_name, service_name, vec![]));
                return;
            }

            let mut all_tasks: Vec<EcsTaskData> = Vec::new();
            for chunk in arns.chunks(100) {
                let mut cmd = Command::new("aws");
                cmd.args(["ecs", "describe-tasks",
                          "--cluster", &cluster_name,
                          "--region", &region, "--profile", &profile,
                          "--output", "json", "--tasks"]);
                for arn in chunk { cmd.arg(arn); }
                cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).stdin(Stdio::null());

                if let Ok(o) = cmd.output().await {
                    if o.status.success() {
                        let v: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap_or_default();
                        if let Some(tasks) = v["tasks"].as_array() {
                            for t in tasks {
                                let task_arn = t["taskArn"].as_str().unwrap_or("");
                                let task_id = task_arn.split('/').last().unwrap_or(task_arn).to_string();
                                let td_full = t["taskDefinitionArn"].as_str().unwrap_or("");
                                let td = td_full.split('/').last().unwrap_or(td_full).to_string();
                                let containers: Vec<EcsContainerData> = t["containers"]
                                    .as_array()
                                    .unwrap_or(&vec![])
                                    .iter()
                                    .map(|c| EcsContainerData {
                                        name:   c["name"].as_str().unwrap_or("?").to_string(),
                                        image:  c["image"].as_str().unwrap_or("").to_string(),
                                        status: c["lastStatus"].as_str().unwrap_or("UNKNOWN").to_string(),
                                    })
                                    .collect();
                                all_tasks.push(EcsTaskData {
                                    task_id,
                                    status:          t["lastStatus"].as_str().unwrap_or("UNKNOWN").to_string(),
                                    launch_type:     t["launchType"].as_str().unwrap_or("").to_string(),
                                    task_definition: td,
                                    containers,
                                });
                            }
                        }
                    }
                }
            }
            let _ = tx.send(FetchResult::EcsTasks(cluster_name, service_name, all_tasks));
        });
    }

    // ── EKS fetching ──────────────────────────────────────────────────

    pub fn fetch_eks_clusters(&mut self) {
        let profile = match self.active_profile() {
            Some(p) => p.to_string(),
            None => return,
        };
        let region = self.active_region().to_string();
        let tx = self.fetch_tx.clone();

        self.loading_eks_clusters = true;
        self.eks_clusters.clear();
        self.eks_nodegroups.clear();
        self.eks_nodegroups_for.clear();

        tokio::spawn(async move {
            let list_out = Command::new("aws")
                .args(["eks", "list-clusters",
                       "--region", &region, "--profile", &profile,
                       "--output", "json"])
                .stdout(Stdio::piped()).stderr(Stdio::piped()).stdin(Stdio::null())
                .output().await;

            let names: Vec<String> = match list_out {
                Ok(o) if o.status.success() => {
                    let v: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap_or_default();
                    v["clusters"].as_array().unwrap_or(&vec![])
                        .iter().filter_map(|n| n.as_str().map(String::from)).collect()
                }
                Ok(o) => {
                    let _ = tx.send(FetchResult::Error(String::from_utf8_lossy(&o.stderr).trim().to_string()));
                    return;
                }
                Err(e) => { let _ = tx.send(FetchResult::Error(e.to_string())); return; }
            };

            if names.is_empty() {
                let _ = tx.send(FetchResult::EksClusters(vec![]));
                return;
            }

            let mut clusters: Vec<EksCluster> = Vec::new();
            for name in &names {
                let out = Command::new("aws")
                    .args(["eks", "describe-cluster",
                           "--name", name,
                           "--region", &region, "--profile", &profile,
                           "--output", "json"])
                    .stdout(Stdio::piped()).stderr(Stdio::piped()).stdin(Stdio::null())
                    .output().await;

                if let Ok(o) = out {
                    if o.status.success() {
                        let v: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap_or_default();
                        let c = &v["cluster"];
                        clusters.push(EksCluster {
                            name:    c["name"].as_str().unwrap_or(name).to_string(),
                            status:  c["status"].as_str().unwrap_or("UNKNOWN").to_string(),
                            version: c["version"].as_str().unwrap_or("?").to_string(),
                        });
                    }
                }
            }
            let _ = tx.send(FetchResult::EksClusters(clusters));
        });
    }

    pub fn fetch_eks_nodegroups(&mut self) {
        let cluster = match self.eks_clusters.get(self.selected_eks_cluster) {
            Some(c) => c.name.clone(),
            None => return,
        };
        let profile = match self.active_profile() {
            Some(p) => p.to_string(),
            None => return,
        };
        let region = self.active_region().to_string();
        let tx = self.fetch_tx.clone();

        self.loading_eks_nodegroups = true;
        self.eks_nodegroups.clear();

        tokio::spawn(async move {
            let list_out = Command::new("aws")
                .args(["eks", "list-nodegroups",
                       "--cluster-name", &cluster,
                       "--region", &region, "--profile", &profile,
                       "--output", "json"])
                .stdout(Stdio::piped()).stderr(Stdio::piped()).stdin(Stdio::null())
                .output().await;

            let ng_names: Vec<String> = match list_out {
                Ok(o) if o.status.success() => {
                    let v: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap_or_default();
                    v["nodegroups"].as_array().unwrap_or(&vec![])
                        .iter().filter_map(|n| n.as_str().map(String::from)).collect()
                }
                Ok(o) => {
                    let _ = tx.send(FetchResult::Error(String::from_utf8_lossy(&o.stderr).trim().to_string()));
                    return;
                }
                Err(e) => { let _ = tx.send(FetchResult::Error(e.to_string())); return; }
            };

            if ng_names.is_empty() {
                let _ = tx.send(FetchResult::EksNodegroups(cluster, vec![]));
                return;
            }

            let mut nodegroups: Vec<EksNodegroup> = Vec::new();
            for ng_name in &ng_names {
                let out = Command::new("aws")
                    .args(["eks", "describe-nodegroup",
                           "--cluster-name", &cluster,
                           "--nodegroup-name", ng_name,
                           "--region", &region, "--profile", &profile,
                           "--output", "json"])
                    .stdout(Stdio::piped()).stderr(Stdio::piped()).stdin(Stdio::null())
                    .output().await;

                if let Ok(o) = out {
                    if o.status.success() {
                        let v: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap_or_default();
                        let ng = &v["nodegroup"];
                        let scaling = &ng["scalingConfig"];
                        let empty_arr = vec![];
                        let types: Vec<&str> = ng["instanceTypes"].as_array()
                            .unwrap_or(&empty_arr)
                            .iter().filter_map(|t| t.as_str()).collect();
                        nodegroups.push(EksNodegroup {
                            name:           ng["nodegroupName"].as_str().unwrap_or(ng_name).to_string(),
                            status:         ng["status"].as_str().unwrap_or("UNKNOWN").to_string(),
                            desired:        scaling["desiredSize"].as_i64().unwrap_or(0),
                            min:            scaling["minSize"].as_i64().unwrap_or(0),
                            max:            scaling["maxSize"].as_i64().unwrap_or(0),
                            instance_types: if types.is_empty() { "?".to_string() } else { types.join(", ") },
                        });
                    }
                }
            }
            let _ = tx.send(FetchResult::EksNodegroups(cluster, nodegroups));
        });
    }

    // ── Helpers ───────────────────────────────────────────────────────

    pub fn fetch_clusters(&mut self) {
        match self.sub_tab {
            ContainersSubTab::Ecs => self.fetch_ecs_clusters(),
            ContainersSubTab::Eks => self.fetch_eks_clusters(),
        }
    }

    pub fn fetch_detail_for_selected(&mut self) {
        match self.sub_tab {
            ContainersSubTab::Ecs => self.ecs_toggle_selected(),
            ContainersSubTab::Eks => self.fetch_eks_nodegroups(),
        }
    }
}
