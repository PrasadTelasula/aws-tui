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
    ClusterList,
    DetailList,
}

// ─── ECS data types ───────────────────────────────────────────────────────────

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
    pub pending: i64,
    pub task_definition: String,
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
    EcsServices(String, Vec<EcsService>),       // cluster_name, services
    EksClusters(Vec<EksCluster>),
    EksNodegroups(String, Vec<EksNodegroup>),   // cluster_name, nodegroups
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

    // ECS
    pub ecs_clusters: Vec<EcsCluster>,
    pub selected_ecs_cluster: usize,
    pub loading_ecs_clusters: bool,
    pub ecs_services: Vec<EcsService>,
    pub selected_ecs_service: usize,
    pub loading_ecs_services: bool,
    pub ecs_services_for: String,   // which cluster owns current services list

    // EKS
    pub eks_clusters: Vec<EksCluster>,
    pub selected_eks_cluster: usize,
    pub loading_eks_clusters: bool,
    pub eks_nodegroups: Vec<EksNodegroup>,
    pub selected_eks_nodegroup: usize,
    pub loading_eks_nodegroups: bool,
    pub eks_nodegroups_for: String, // which cluster owns current nodegroups list

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
            ecs_clusters: Vec::new(),
            selected_ecs_cluster: 0,
            loading_ecs_clusters: false,
            ecs_services: Vec::new(),
            selected_ecs_service: 0,
            loading_ecs_services: false,
            ecs_services_for: String::new(),
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

    // ── List navigation ───────────────────────────────────────────────

    pub fn next_cluster(&mut self) {
        let len = match self.sub_tab {
            ContainersSubTab::Ecs => self.ecs_clusters.len(),
            ContainersSubTab::Eks => self.eks_clusters.len(),
        };
        if len == 0 { return; }
        match self.sub_tab {
            ContainersSubTab::Ecs => self.selected_ecs_cluster = (self.selected_ecs_cluster + 1) % len,
            ContainersSubTab::Eks => self.selected_eks_cluster = (self.selected_eks_cluster + 1) % len,
        }
    }

    pub fn prev_cluster(&mut self) {
        let len = match self.sub_tab {
            ContainersSubTab::Ecs => self.ecs_clusters.len(),
            ContainersSubTab::Eks => self.eks_clusters.len(),
        };
        if len == 0 { return; }
        match self.sub_tab {
            ContainersSubTab::Ecs => {
                self.selected_ecs_cluster = if self.selected_ecs_cluster == 0 { len - 1 } else { self.selected_ecs_cluster - 1 };
            }
            ContainersSubTab::Eks => {
                self.selected_eks_cluster = if self.selected_eks_cluster == 0 { len - 1 } else { self.selected_eks_cluster - 1 };
            }
        }
    }

    pub fn next_detail(&mut self) {
        match self.sub_tab {
            ContainersSubTab::Ecs => {
                if !self.ecs_services.is_empty() {
                    self.selected_ecs_service = (self.selected_ecs_service + 1) % self.ecs_services.len();
                }
            }
            ContainersSubTab::Eks => {
                if !self.eks_nodegroups.is_empty() {
                    self.selected_eks_nodegroup = (self.selected_eks_nodegroup + 1) % self.eks_nodegroups.len();
                }
            }
        }
    }

    pub fn prev_detail(&mut self) {
        match self.sub_tab {
            ContainersSubTab::Ecs => {
                if !self.ecs_services.is_empty() {
                    self.selected_ecs_service = if self.selected_ecs_service == 0 {
                        self.ecs_services.len() - 1
                    } else {
                        self.selected_ecs_service - 1
                    };
                }
            }
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
            ContainersFocus::RegionList  => ContainersFocus::ClusterList,
            ContainersFocus::ClusterList => ContainersFocus::DetailList,
            ContainersFocus::DetailList  => ContainersFocus::RegionList,
        };
    }

    // ── Tick — drain async results ────────────────────────────────────

    pub fn tick(&mut self) {
        while let Ok(result) = self.fetch_rx.try_recv() {
            match result {
                FetchResult::EcsClusters(clusters) => {
                    self.ecs_clusters = clusters;
                    self.selected_ecs_cluster = 0;
                    self.loading_ecs_clusters = false;
                    self.ecs_services.clear();
                    self.ecs_services_for.clear();
                    self.last_error = None;
                }
                FetchResult::EcsServices(cluster, services) => {
                    self.ecs_services = services;
                    self.selected_ecs_service = 0;
                    self.loading_ecs_services = false;
                    self.ecs_services_for = cluster;
                    self.last_error = None;
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
                    self.loading_ecs_services = false;
                    self.loading_eks_clusters = false;
                    self.loading_eks_nodegroups = false;
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
        self.ecs_clusters.clear();
        self.ecs_services.clear();
        self.ecs_services_for.clear();

        tokio::spawn(async move {
            // Step 1: list ARNs
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

            // Step 2: describe clusters
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

    pub fn fetch_ecs_services(&mut self) {
        let cluster = match self.ecs_clusters.get(self.selected_ecs_cluster) {
            Some(c) => c.name.clone(),
            None => return,
        };
        let profile = match self.active_profile() {
            Some(p) => p.to_string(),
            None => return,
        };
        let region = self.active_region().to_string();
        let tx = self.fetch_tx.clone();

        self.loading_ecs_services = true;
        self.ecs_services.clear();

        tokio::spawn(async move {
            // Step 1: list service ARNs
            let list_out = Command::new("aws")
                .args(["ecs", "list-services",
                       "--cluster", &cluster,
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
                let _ = tx.send(FetchResult::EcsServices(cluster, vec![]));
                return;
            }

            // Step 2: describe services (max 10 per call — AWS limit)
            let mut all_services: Vec<EcsService> = Vec::new();
            for chunk in arns.chunks(10) {
                let mut cmd = Command::new("aws");
                cmd.args(["ecs", "describe-services",
                          "--cluster", &cluster,
                          "--region", &region, "--profile", &profile,
                          "--output", "json", "--services"]);
                for arn in chunk { cmd.arg(arn); }
                cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).stdin(Stdio::null());

                if let Ok(o) = cmd.output().await {
                    if o.status.success() {
                        let v: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap_or_default();
                        if let Some(svcs) = v["services"].as_array() {
                            for s in svcs {
                                // Shorten task definition: strip registry prefix, keep name:rev
                                let td_full = s["taskDefinition"].as_str().unwrap_or("");
                                let td = td_full.split('/').last().unwrap_or(td_full).to_string();
                                all_services.push(EcsService {
                                    name:            s["serviceName"].as_str().unwrap_or("unknown").to_string(),
                                    status:          s["status"].as_str().unwrap_or("UNKNOWN").to_string(),
                                    desired:         s["desiredCount"].as_i64().unwrap_or(0),
                                    running:         s["runningCount"].as_i64().unwrap_or(0),
                                    pending:         s["pendingCount"].as_i64().unwrap_or(0),
                                    task_definition: td,
                                });
                            }
                        }
                    }
                }
            }
            let _ = tx.send(FetchResult::EcsServices(cluster, all_services));
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
            // Step 1: list cluster names
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

            // Step 2: describe each cluster
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
            // Step 1: list nodegroup names
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

            // Step 2: describe each nodegroup
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

    /// Fetch clusters for the currently active sub-tab.
    pub fn fetch_clusters(&mut self) {
        match self.sub_tab {
            ContainersSubTab::Ecs => self.fetch_ecs_clusters(),
            ContainersSubTab::Eks => self.fetch_eks_clusters(),
        }
    }

    /// Load detail (services / nodegroups) for the selected cluster.
    pub fn fetch_detail_for_selected(&mut self) {
        match self.sub_tab {
            ContainersSubTab::Ecs => self.fetch_ecs_services(),
            ContainersSubTab::Eks => self.fetch_eks_nodegroups(),
        }
    }
}
