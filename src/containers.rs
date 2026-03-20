use crate::instances::{InstanceInfoPopup, InfoTab, REGIONS};
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
    EksNodegroups(String, Vec<EksNodegroup>),        // cluster_name, nodegroups
    EksClusters(Vec<EksCluster>),
    EcsInfoPopup(InstanceInfoPopup),
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
    pub ecs_search_active: bool,
    pub ecs_search_query: String,
    pub ecs_filtered_indices: Vec<usize>,

    // EKS
    pub eks_clusters: Vec<EksCluster>,
    pub selected_eks_cluster: usize,
    pub loading_eks_clusters: bool,
    pub eks_nodegroups: Vec<EksNodegroup>,
    pub selected_eks_nodegroup: usize,
    pub loading_eks_nodegroups: bool,
    pub eks_nodegroups_for: String,

    // Info popup (i key)
    pub show_info_popup: bool,
    pub info_popup: Option<InstanceInfoPopup>,

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
            ecs_search_active: false,
            ecs_search_query: String::new(),
            ecs_filtered_indices: Vec::new(),
            eks_clusters: Vec::new(),
            selected_eks_cluster: 0,
            loading_eks_clusters: false,
            eks_nodegroups: Vec::new(),
            selected_eks_nodegroup: 0,
            loading_eks_nodegroups: false,
            eks_nodegroups_for: String::new(),
            show_info_popup: false,
            info_popup: None,
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
        if self.ecs_tree.is_empty() { return; }
        if !self.ecs_filtered_indices.is_empty() {
            let pos = self.ecs_filtered_indices.iter()
                .position(|&i| i == self.selected_ecs_tree)
                .unwrap_or(0);
            let next = (pos + 1) % self.ecs_filtered_indices.len();
            self.selected_ecs_tree = self.ecs_filtered_indices[next];
        } else if self.ecs_search_query.is_empty() {
            self.selected_ecs_tree = (self.selected_ecs_tree + 1) % self.ecs_tree.len();
        }
    }

    pub fn ecs_prev_item(&mut self) {
        if self.ecs_tree.is_empty() { return; }
        if !self.ecs_filtered_indices.is_empty() {
            let pos = self.ecs_filtered_indices.iter()
                .position(|&i| i == self.selected_ecs_tree)
                .unwrap_or(0);
            let prev = if pos == 0 { self.ecs_filtered_indices.len() - 1 } else { pos - 1 };
            self.selected_ecs_tree = self.ecs_filtered_indices[prev];
        } else if self.ecs_search_query.is_empty() {
            self.selected_ecs_tree = if self.selected_ecs_tree == 0 {
                self.ecs_tree.len() - 1
            } else {
                self.selected_ecs_tree - 1
            };
        }
    }

    pub fn update_ecs_search(&mut self) {
        if self.ecs_search_query.is_empty() {
            self.ecs_filtered_indices.clear();
            return;
        }
        let query = self.ecs_search_query.to_lowercase();
        self.ecs_filtered_indices = self.ecs_tree.iter().enumerate()
            .filter(|(_, item)| item.name.to_lowercase().contains(&query))
            .map(|(i, _)| i)
            .collect();
        if !self.ecs_filtered_indices.is_empty()
            && !self.ecs_filtered_indices.contains(&self.selected_ecs_tree)
        {
            self.selected_ecs_tree = self.ecs_filtered_indices[0];
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
        self.update_ecs_search();
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
                FetchResult::EcsInfoPopup(popup) => {
                    self.info_popup = Some(popup);
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
        self.ecs_search_query.clear();
        self.ecs_search_active = false;
        self.ecs_filtered_indices.clear();

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

    // ── ECS info popup ────────────────────────────────────────────────

    pub fn fetch_ecs_info(&mut self) {
        let item = match self.ecs_tree.get(self.selected_ecs_tree) {
            Some(i) => i.clone(),
            None => return,
        };
        let profile = match self.active_profile() {
            Some(p) => p.to_string(),
            None => return,
        };
        let region = self.active_region().to_string();
        let tx = self.fetch_tx.clone();

        self.show_info_popup = true;
        self.info_popup = Some(InstanceInfoPopup::new_loading());

        tokio::spawn(async move {
            let popup = match item.kind {
                EcsTreeItemKind::Cluster => {
                    fetch_cluster_info(&item.name, &profile, &region).await
                }
                EcsTreeItemKind::Service => {
                    fetch_service_info(&item.cluster_name, &item.name, &profile, &region).await
                }
                EcsTreeItemKind::Task => {
                    fetch_task_info(&item.cluster_name, &item.name, &profile, &region).await
                }
                EcsTreeItemKind::Container => {
                    // Containers live inside tasks — re-fetch the task and extract this container
                    fetch_container_info(&item.cluster_name, &item.service_name, &item.name, &item.extra, &profile, &region).await
                }
            };
            let _ = tx.send(FetchResult::EcsInfoPopup(popup));
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

// ─── ECS info popup async helpers ────────────────────────────────────────────

fn make_error_popup(msg: String) -> InstanceInfoPopup {
    InstanceInfoPopup {
        loading: false, tab: InfoTab::Human, scroll: 0,
        human_lines: vec![format!("  Error: {}", msg)],
        json_lines:  vec![format!("  Error: {}", msg)],
        search_query: String::new(), search_active: false,
        search_matches: Vec::new(), search_match_idx: 0,
    }
}

fn make_popup(human: Vec<String>, raw_json: &str) -> InstanceInfoPopup {
    let json_lines = match serde_json::from_str::<serde_json::Value>(raw_json) {
        Ok(v) => serde_json::to_string_pretty(&v)
            .unwrap_or_else(|_| raw_json.to_string())
            .lines().map(|l| l.to_string()).collect(),
        Err(_) => raw_json.lines().map(|l| l.to_string()).collect(),
    };
    InstanceInfoPopup {
        loading: false, tab: InfoTab::Human, scroll: 0,
        human_lines: human, json_lines,
        search_query: String::new(), search_active: false,
        search_matches: Vec::new(), search_match_idx: 0,
    }
}

async fn fetch_cluster_info(name: &str, profile: &str, region: &str) -> InstanceInfoPopup {
    let out = Command::new("aws")
        .args(["ecs", "describe-clusters",
               "--clusters", name,
               "--include", "STATISTICS", "TAGS",
               "--region", region, "--profile", profile,
               "--output", "json"])
        .stdout(Stdio::piped()).stderr(Stdio::piped()).stdin(Stdio::null())
        .output().await;

    match out {
        Ok(o) if o.status.success() => {
            let raw = String::from_utf8_lossy(&o.stdout).to_string();
            let human = format_cluster_human(&raw);
            make_popup(human, &raw)
        }
        Ok(o) => make_error_popup(String::from_utf8_lossy(&o.stderr).trim().to_string()),
        Err(e) => make_error_popup(e.to_string()),
    }
}

async fn fetch_service_info(cluster: &str, service: &str, profile: &str, region: &str) -> InstanceInfoPopup {
    let out = Command::new("aws")
        .args(["ecs", "describe-services",
               "--cluster", cluster,
               "--services", service,
               "--region", region, "--profile", profile,
               "--output", "json"])
        .stdout(Stdio::piped()).stderr(Stdio::piped()).stdin(Stdio::null())
        .output().await;

    match out {
        Ok(o) if o.status.success() => {
            let raw = String::from_utf8_lossy(&o.stdout).to_string();
            let human = format_service_human(&raw);
            make_popup(human, &raw)
        }
        Ok(o) => make_error_popup(String::from_utf8_lossy(&o.stderr).trim().to_string()),
        Err(e) => make_error_popup(e.to_string()),
    }
}

async fn fetch_task_info(cluster: &str, task_id: &str, profile: &str, region: &str) -> InstanceInfoPopup {
    let out = Command::new("aws")
        .args(["ecs", "describe-tasks",
               "--cluster", cluster,
               "--tasks", task_id,
               "--region", region, "--profile", profile,
               "--output", "json"])
        .stdout(Stdio::piped()).stderr(Stdio::piped()).stdin(Stdio::null())
        .output().await;

    match out {
        Ok(o) if o.status.success() => {
            let raw = String::from_utf8_lossy(&o.stdout).to_string();
            let human = format_task_human(&raw, None);
            make_popup(human, &raw)
        }
        Ok(o) => make_error_popup(String::from_utf8_lossy(&o.stderr).trim().to_string()),
        Err(e) => make_error_popup(e.to_string()),
    }
}

async fn fetch_container_info(cluster: &str, service: &str, container_name: &str, image: &str, profile: &str, region: &str) -> InstanceInfoPopup {
    // First find a task for this service, then describe it and extract this container
    let list_out = Command::new("aws")
        .args(["ecs", "list-tasks",
               "--cluster", cluster,
               "--service-name", service,
               "--region", region, "--profile", profile,
               "--output", "json"])
        .stdout(Stdio::piped()).stderr(Stdio::piped()).stdin(Stdio::null())
        .output().await;

    let task_arns: Vec<String> = match list_out {
        Ok(o) if o.status.success() => {
            let v: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap_or_default();
            v["taskArns"].as_array().unwrap_or(&vec![])
                .iter().filter_map(|a| a.as_str().map(String::from)).collect()
        }
        Ok(o) => return make_error_popup(String::from_utf8_lossy(&o.stderr).trim().to_string()),
        Err(e) => return make_error_popup(e.to_string()),
    };

    let task_arn = match task_arns.first() {
        Some(a) => a.clone(),
        None => return make_error_popup("No running tasks found for this service".to_string()),
    };

    let out = Command::new("aws")
        .args(["ecs", "describe-tasks",
               "--cluster", cluster,
               "--tasks", &task_arn,
               "--region", region, "--profile", profile,
               "--output", "json"])
        .stdout(Stdio::piped()).stderr(Stdio::piped()).stdin(Stdio::null())
        .output().await;

    match out {
        Ok(o) if o.status.success() => {
            let raw = String::from_utf8_lossy(&o.stdout).to_string();
            let human = format_task_human(&raw, Some(container_name));
            // For JSON, show just this container's data
            let json_raw = extract_container_json(&raw, container_name)
                .unwrap_or_else(|| format!("{{ \"name\": \"{}\", \"image\": \"{}\" }}", container_name, image));
            make_popup(human, &json_raw)
        }
        Ok(o) => make_error_popup(String::from_utf8_lossy(&o.stderr).trim().to_string()),
        Err(e) => make_error_popup(e.to_string()),
    }
}

fn extract_container_json(task_json: &str, container_name: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(task_json).ok()?;
    let containers = v["tasks"][0]["containers"].as_array()?;
    let c = containers.iter().find(|c| c["name"].as_str() == Some(container_name))?;
    serde_json::to_string_pretty(c).ok()
}

// ─── Human formatters ─────────────────────────────────────────────────────────

fn s(v: &serde_json::Value) -> String {
    v.as_str().unwrap_or("-").to_string()
}

fn section(lines: &mut Vec<String>, title: &str) {
    lines.push(format!("── {} {}", title, "─".repeat(48_usize.saturating_sub(title.len() + 4))));
}

fn field(lines: &mut Vec<String>, label: &str, value: &str) {
    if value != "-" && !value.is_empty() {
        lines.push(format!("  {:<24}{}", label, value));
    }
}

fn format_cluster_human(json: &str) -> Vec<String> {
    let root: serde_json::Value = match serde_json::from_str(json) {
        Ok(v) => v,
        Err(e) => return vec![format!("  Parse error: {}", e)],
    };
    let c = match root["clusters"].as_array().and_then(|a| a.first()) {
        Some(v) => v.clone(),
        None => return vec!["  No cluster data found.".to_string()],
    };
    let mut lines: Vec<String> = Vec::new();

    section(&mut lines, "Identity");
    field(&mut lines, "Name",   &s(&c["clusterName"]));
    field(&mut lines, "Status", &s(&c["status"]));
    field(&mut lines, "ARN",    &s(&c["clusterArn"]));
    lines.push(String::new());

    section(&mut lines, "Capacity");
    field(&mut lines, "Active Services",        &c["activeServicesCount"].as_i64().unwrap_or(0).to_string());
    field(&mut lines, "Running Tasks",          &c["runningTasksCount"].as_i64().unwrap_or(0).to_string());
    field(&mut lines, "Pending Tasks",          &c["pendingTasksCount"].as_i64().unwrap_or(0).to_string());
    field(&mut lines, "Container Instances",    &c["registeredContainerInstancesCount"].as_i64().unwrap_or(0).to_string());
    lines.push(String::new());

    if let Some(providers) = c["capacityProviders"].as_array() {
        if !providers.is_empty() {
            section(&mut lines, "Capacity Providers");
            for p in providers {
                if let Some(name) = p.as_str() {
                    lines.push(format!("  {}", name));
                }
            }
            lines.push(String::new());
        }
    }

    if let Some(settings) = c["settings"].as_array() {
        if !settings.is_empty() {
            section(&mut lines, "Settings");
            for setting in settings {
                field(&mut lines, &s(&setting["name"]), &s(&setting["value"]));
            }
            lines.push(String::new());
        }
    }

    if let Some(tags) = c["tags"].as_array() {
        if !tags.is_empty() {
            section(&mut lines, "Tags");
            for tag in tags {
                lines.push(format!("  {:<24}{}", s(&tag["key"]), s(&tag["value"])));
            }
            lines.push(String::new());
        }
    }

    lines
}

fn format_service_human(json: &str) -> Vec<String> {
    let root: serde_json::Value = match serde_json::from_str(json) {
        Ok(v) => v,
        Err(e) => return vec![format!("  Parse error: {}", e)],
    };
    let svc = match root["services"].as_array().and_then(|a| a.first()) {
        Some(v) => v.clone(),
        None => return vec!["  No service data found.".to_string()],
    };
    let mut lines: Vec<String> = Vec::new();

    section(&mut lines, "Identity");
    field(&mut lines, "Name",            &s(&svc["serviceName"]));
    field(&mut lines, "Status",          &s(&svc["status"]));
    field(&mut lines, "ARN",             &s(&svc["serviceArn"]));
    field(&mut lines, "Cluster",         svc["clusterArn"].as_str().and_then(|a| a.split('/').last()).unwrap_or("-"));
    field(&mut lines, "Launch Type",     &s(&svc["launchType"]));
    field(&mut lines, "Scheduling",      &s(&svc["schedulingStrategy"]));
    field(&mut lines, "Created",         &s(&svc["createdAt"]));
    lines.push(String::new());

    section(&mut lines, "Scale");
    field(&mut lines, "Desired",  &svc["desiredCount"].as_i64().unwrap_or(0).to_string());
    field(&mut lines, "Running",  &svc["runningCount"].as_i64().unwrap_or(0).to_string());
    field(&mut lines, "Pending",  &svc["pendingCount"].as_i64().unwrap_or(0).to_string());
    field(&mut lines, "Minimum Healthy", &format!("{}%", svc["deploymentConfiguration"]["minimumHealthyPercent"].as_i64().unwrap_or(0)));
    field(&mut lines, "Maximum",         &format!("{}%", svc["deploymentConfiguration"]["maximumPercent"].as_i64().unwrap_or(0)));
    lines.push(String::new());

    let td = svc["taskDefinition"].as_str().and_then(|t| t.split('/').last()).unwrap_or("-");
    section(&mut lines, "Task Definition");
    field(&mut lines, "Task Definition", td);
    lines.push(String::new());

    if let Some(net) = svc.get("networkConfiguration").and_then(|n| n["awsvpcConfiguration"].as_object()) {
        section(&mut lines, "Network");
        if let Some(subnets) = net.get("subnets").and_then(|v| v.as_array()) {
            for sn in subnets { field(&mut lines, "Subnet", sn.as_str().unwrap_or("-")); }
        }
        if let Some(sgs) = net.get("securityGroups").and_then(|v| v.as_array()) {
            for sg in sgs { field(&mut lines, "Security Group", sg.as_str().unwrap_or("-")); }
        }
        field(&mut lines, "Assign Public IP", net.get("assignPublicIp").and_then(|v| v.as_str()).unwrap_or("-"));
        lines.push(String::new());
    }

    if let Some(lbs) = svc["loadBalancers"].as_array() {
        if !lbs.is_empty() {
            section(&mut lines, "Load Balancers");
            for lb in lbs {
                let tg = lb["targetGroupArn"].as_str().and_then(|a| a.split('/').nth(1)).unwrap_or("-");
                field(&mut lines, "Target Group", tg);
                field(&mut lines, "Container Name", &s(&lb["containerName"]));
                field(&mut lines, "Container Port", &lb["containerPort"].as_i64().unwrap_or(0).to_string());
            }
            lines.push(String::new());
        }
    }

    if let Some(deployments) = svc["deployments"].as_array() {
        if !deployments.is_empty() {
            section(&mut lines, "Deployments");
            for d in deployments {
                field(&mut lines, "Status",  &s(&d["status"]));
                field(&mut lines, "Desired", &d["desiredCount"].as_i64().unwrap_or(0).to_string());
                field(&mut lines, "Running", &d["runningCount"].as_i64().unwrap_or(0).to_string());
                field(&mut lines, "Updated", &s(&d["updatedAt"]));
                lines.push(String::new());
            }
        }
    }

    if let Some(tags) = svc["tags"].as_array() {
        if !tags.is_empty() {
            section(&mut lines, "Tags");
            for tag in tags {
                lines.push(format!("  {:<24}{}", s(&tag["key"]), s(&tag["value"])));
            }
            lines.push(String::new());
        }
    }

    lines
}

fn format_task_human(json: &str, only_container: Option<&str>) -> Vec<String> {
    let root: serde_json::Value = match serde_json::from_str(json) {
        Ok(v) => v,
        Err(e) => return vec![format!("  Parse error: {}", e)],
    };
    let task = match root["tasks"].as_array().and_then(|a| a.first()) {
        Some(v) => v.clone(),
        None => return vec!["  No task data found.".to_string()],
    };
    let mut lines: Vec<String> = Vec::new();

    let task_id = task["taskArn"].as_str()
        .and_then(|a| a.split('/').last()).unwrap_or("-");

    if only_container.is_none() {
        // Full task view
        section(&mut lines, "Identity");
        field(&mut lines, "Task ID",        task_id);
        field(&mut lines, "Status",         &s(&task["lastStatus"]));
        field(&mut lines, "Desired Status", &s(&task["desiredStatus"]));
        field(&mut lines, "Launch Type",    &s(&task["launchType"]));
        field(&mut lines, "Group",          &s(&task["group"]));
        field(&mut lines, "Platform",       &s(&task["platformVersion"]));
        lines.push(String::new());

        section(&mut lines, "Resources");
        field(&mut lines, "CPU",    &s(&task["cpu"]));
        field(&mut lines, "Memory", &s(&task["memory"]));
        lines.push(String::new());

        section(&mut lines, "Timing");
        field(&mut lines, "Created At", &s(&task["createdAt"]));
        field(&mut lines, "Started At", &s(&task["startedAt"]));
        field(&mut lines, "Pull Start", &s(&task["pullStartedAt"]));
        field(&mut lines, "Pull Stop",  &s(&task["pullStoppedAt"]));
        lines.push(String::new());

        // Network (ENI attachment)
        if let Some(attachments) = task["attachments"].as_array() {
            for att in attachments {
                if att["type"].as_str() == Some("ElasticNetworkInterface") {
                    section(&mut lines, "Network (ENI)");
                    if let Some(details) = att["details"].as_array() {
                        for d in details {
                            if let (Some(name), Some(val)) = (d["name"].as_str(), d["value"].as_str()) {
                                field(&mut lines, name, val);
                            }
                        }
                    }
                    lines.push(String::new());
                }
            }
        }
    }

    // Containers section (all containers, or just the selected one)
    if let Some(containers) = task["containers"].as_array() {
        let to_show: Vec<&serde_json::Value> = if let Some(name) = only_container {
            containers.iter().filter(|c| c["name"].as_str() == Some(name)).collect()
        } else {
            containers.iter().collect()
        };

        for c in to_show {
            let cname = s(&c["name"]);
            section(&mut lines, &format!("Container: {}", cname));
            field(&mut lines, "Status",  &s(&c["lastStatus"]));
            field(&mut lines, "Image",   &s(&c["image"]));
            field(&mut lines, "CPU",     &s(&c["cpu"]));
            field(&mut lines, "Memory",  &s(&c["memory"]));
            if let Some(exit_code) = c["exitCode"].as_i64() {
                field(&mut lines, "Exit Code", &exit_code.to_string());
            }
            if let Some(reason) = c["reason"].as_str() {
                if !reason.is_empty() {
                    field(&mut lines, "Reason", reason);
                }
            }
            field(&mut lines, "Health",  &s(&c["healthStatus"]));
            // Network interfaces
            if let Some(ifaces) = c["networkInterfaces"].as_array() {
                for iface in ifaces {
                    field(&mut lines, "Private IP",  &s(&iface["privateIpv4Address"]));
                    field(&mut lines, "IPv6",        &s(&iface["ipv6Address"]));
                }
            }
            // Port mappings
            if let Some(bindings) = c["networkBindings"].as_array() {
                for b in bindings {
                    field(&mut lines, "Port Binding",
                        &format!("{}:{} ({})", s(&b["hostPort"]), s(&b["containerPort"]), s(&b["protocol"])));
                }
            }
            lines.push(String::new());
        }
    }

    lines
}
