use super::*;
use crate::app::App;
use crate::containers::{ContainersFocus, ContainersSubTab};

// Icons specific to this tab
const ICON_CUBE: &str = "\u{f1b2}";    //  cube  (containers / EKS)
const ICON_LAYERS: &str = "\u{f0c8}";  //  th-large (ECS services)
const ICON_COG2: &str = "\u{f085}";    //  cogs  (services)
const ICON_NODES: &str = "\u{f233}";   //  server (nodegroups, reuse)

pub(super) fn draw_containers(f: &mut Frame, area: Rect, app: &App) {
    let cs = &app.containers_state;

    // ── No profiles yet ──────────────────────────────────────────────
    if cs.profiles.is_empty() {
        let outer = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(FG4))
            .style(Style::default().bg(BG));
        let inner = outer.inner(area);
        f.render_widget(outer, area);
        f.render_widget(
            Paragraph::new(vec![
                Line::from(""),
                Line::from(Span::styled(
                    "  No active SSO profiles. Connect via the Sessions tab first.",
                    Style::default().fg(FG3),
                )),
            ]),
            inner,
        );
        return;
    }

    let outer = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(FG4))
        .style(Style::default().bg(BG));
    let inner = outer.inner(area);
    f.render_widget(outer, area);

    // ── Layout: profile bar (1) | sub-tab bar (1) | body (min) ───────
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(1), Constraint::Min(5)])
        .split(inner);

    draw_profile_bar(f, rows[0], app);
    draw_subtab_bar(f, rows[1], app);

    match cs.sub_tab {
        ContainersSubTab::Ecs => draw_ecs_body(f, rows[2], app),
        ContainersSubTab::Eks => draw_eks_body(f, rows[2], app),
    }
}

// ─── Profile bar ─────────────────────────────────────────────────────────────

fn draw_profile_bar(f: &mut Frame, area: Rect, app: &App) {
    let cs = &app.containers_state;
    let mut spans: Vec<Span> = vec![
        Span::styled(format!(" {} ", ICON_KEY), Style::default().fg(FG3)),
    ];
    for (i, p) in cs.profiles.iter().enumerate() {
        let active = i == cs.active_profile_idx;
        spans.push(Span::styled(
            format!(" {} ", p),
            if active {
                Style::default().fg(BG).bg(GREEN).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(FG3)
            },
        ));
        spans.push(Span::styled(" ", Style::default()));
    }
    spans.push(Span::styled(
        format!("    {} {}", ICON_GLOBE, cs.active_region()),
        Style::default().fg(TEAL),
    ));
    spans.push(Span::styled(
        "    Ctrl+H/L profile  r refresh  1/2 ECS/EKS",
        Style::default().fg(FG4),
    ));
    f.render_widget(
        Paragraph::new(Line::from(spans)).style(Style::default().bg(BG_BAR)),
        area,
    );
}

// ─── Sub-tab bar ─────────────────────────────────────────────────────────────

fn draw_subtab_bar(f: &mut Frame, area: Rect, app: &App) {
    let cs = &app.containers_state;

    let ecs_style = if cs.sub_tab == ContainersSubTab::Ecs {
        Style::default().fg(BG).bg(AMBER).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(FG3)
    };
    let eks_style = if cs.sub_tab == ContainersSubTab::Eks {
        Style::default().fg(BG).bg(BLUE).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(FG3)
    };

    let line = Line::from(vec![
        Span::styled(" ", Style::default()),
        Span::styled(format!(" {} ECS ", ICON_LAYERS), ecs_style),
        Span::styled("  ", Style::default()),
        Span::styled(format!(" {} EKS ", ICON_CUBE), eks_style),
    ]);
    f.render_widget(
        Paragraph::new(line).style(Style::default().bg(BG_BAR)),
        area,
    );
}

// ─── ECS body ────────────────────────────────────────────────────────────────

fn draw_ecs_body(f: &mut Frame, area: Rect, app: &App) {
    let cs = &app.containers_state;

    let left_w    = (area.width * 36) / 100;
    let right_w   = area.width.saturating_sub(left_w + 1);
    let left_area = Rect::new(area.x,              area.y, left_w,  area.height);
    let div_area  = Rect::new(area.x + left_w,     area.y, 1,       area.height);
    let right_area= Rect::new(area.x + left_w + 1, area.y, right_w, area.height);

    let div_color = if cs.focus == ContainersFocus::ClusterList { AMBER } else { FG4 };
    let div_lines: Vec<Line> = (0..div_area.height)
        .map(|_| Line::from(Span::styled("│", Style::default().fg(div_color))))
        .collect();
    f.render_widget(Paragraph::new(div_lines), div_area);

    // ── Left: cluster list ────────────────────────────────────────────
    let left_rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(3)])
        .split(left_area);

    // Header
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(format!("  {} ECS Clusters", ICON_LAYERS), Style::default().fg(AMBER).add_modifier(Modifier::BOLD)),
            if cs.loading_ecs_clusters {
                Span::styled(format!("  {} loading…", app.spinner()), Style::default().fg(AMBER))
            } else {
                Span::styled(format!("  {}", cs.ecs_clusters.len()), Style::default().fg(FG3))
            },
        ])),
        left_rows[0],
    );

    let cluster_focused = cs.focus == ContainersFocus::ClusterList;
    let mut items: Vec<ListItem> = Vec::new();

    if cs.ecs_clusters.is_empty() && !cs.loading_ecs_clusters {
        items.push(ListItem::new(Line::from(Span::styled(
            "  no clusters  (press r)",
            Style::default().fg(FG4),
        ))));
    } else {
        for (i, cluster) in cs.ecs_clusters.iter().enumerate() {
            let selected = i == cs.selected_ecs_cluster;
            let sel_bar = if selected && cluster_focused {
                Span::styled("▌", Style::default().fg(AMBER))
            } else {
                Span::styled(" ", Style::default())
            };
            let status_color = status_color(&cluster.status);
            let name_style = if selected && cluster_focused {
                Style::default().fg(FG).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(FG2)
            };
            let mut item = ListItem::new(Line::from(vec![
                sel_bar,
                Span::styled(format!(" {} ", ICON_LAYERS), Style::default().fg(AMBER)),
                Span::styled(cluster.name.clone(), name_style),
                Span::styled(
                    format!("  {} svc", cluster.active_services),
                    Style::default().fg(FG3),
                ),
                Span::styled(
                    format!("  {} run", cluster.running_tasks),
                    Style::default().fg(if cluster.running_tasks > 0 { GREEN } else { FG4 }),
                ),
                if cluster.pending_tasks > 0 {
                    Span::styled(format!("  {} pend", cluster.pending_tasks), Style::default().fg(AMBER))
                } else {
                    Span::raw("")
                },
                Span::styled(
                    format!("  {}", cluster.status),
                    Style::default().fg(status_color),
                ),
            ]));
            if selected && cluster_focused {
                item = item.style(Style::default().bg(BG_HL));
            }
            items.push(item);
        }
    }
    f.render_widget(List::new(items), left_rows[1]);

    // ── Right: service list ───────────────────────────────────────────
    let right_rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(3)])
        .split(right_area);

    // Header showing which cluster's services are displayed
    let svc_header = if !cs.ecs_services_for.is_empty() {
        format!("  {} Services — {}", ICON_COG2, cs.ecs_services_for)
    } else {
        format!("  {} Services", ICON_COG2)
    };
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(svc_header, Style::default().fg(TEAL).add_modifier(Modifier::BOLD)),
            if cs.loading_ecs_services {
                Span::styled(format!("  {} loading…", app.spinner()), Style::default().fg(AMBER))
            } else if !cs.ecs_services.is_empty() {
                Span::styled(format!("  {}", cs.ecs_services.len()), Style::default().fg(FG3))
            } else {
                Span::raw("")
            },
        ])),
        right_rows[0],
    );

    let detail_focused = cs.focus == ContainersFocus::DetailList;
    let mut svc_items: Vec<ListItem> = Vec::new();

    if let Some(ref err) = cs.last_error {
        svc_items.push(ListItem::new(Line::from(Span::styled(
            format!("  ✗ {}", err),
            Style::default().fg(RED),
        ))));
    } else if cs.ecs_clusters.is_empty() && !cs.loading_ecs_clusters {
        // nothing to show
    } else if cs.ecs_services.is_empty() && !cs.loading_ecs_services {
        let hint = if cs.ecs_services_for.is_empty() {
            "  Select a cluster and press Enter"
        } else {
            "  No services in this cluster"
        };
        svc_items.push(ListItem::new(Line::from(Span::styled(hint, Style::default().fg(FG4)))));
    } else {
        for (i, svc) in cs.ecs_services.iter().enumerate() {
            let selected = i == cs.selected_ecs_service;
            let sel_bar = if selected && detail_focused {
                Span::styled("▌", Style::default().fg(TEAL))
            } else {
                Span::styled(" ", Style::default())
            };
            let status_color = status_color(&svc.status);
            let count_color = if svc.running == svc.desired { GREEN } else { AMBER };
            let name_style = if selected && detail_focused {
                Style::default().fg(FG).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(FG2)
            };
            let mut item = ListItem::new(Line::from(vec![
                sel_bar,
                Span::styled(format!(" {} ", ICON_COG2), Style::default().fg(TEAL)),
                Span::styled(svc.name.clone(), name_style),
                Span::styled(
                    format!("  {}/{}", svc.running, svc.desired),
                    Style::default().fg(count_color),
                ),
                if svc.pending > 0 {
                    Span::styled(format!(" +{} pend", svc.pending), Style::default().fg(AMBER))
                } else {
                    Span::raw("")
                },
                Span::styled(
                    format!("  {}", svc.status),
                    Style::default().fg(status_color),
                ),
                Span::styled(
                    format!("  {}", svc.task_definition),
                    Style::default().fg(FG4),
                ),
            ]));
            if selected && detail_focused {
                item = item.style(Style::default().bg(BG_HL));
            }
            svc_items.push(item);
        }
    }
    f.render_widget(List::new(svc_items), right_rows[1]);

    // Region dropdown
    if cs.region_dropdown_open {
        draw_region_dropdown(f, area, app);
    }
}

// ─── EKS body ────────────────────────────────────────────────────────────────

fn draw_eks_body(f: &mut Frame, area: Rect, app: &App) {
    let cs = &app.containers_state;

    let left_w    = (area.width * 36) / 100;
    let right_w   = area.width.saturating_sub(left_w + 1);
    let left_area = Rect::new(area.x,              area.y, left_w,  area.height);
    let div_area  = Rect::new(area.x + left_w,     area.y, 1,       area.height);
    let right_area= Rect::new(area.x + left_w + 1, area.y, right_w, area.height);

    let div_color = if cs.focus == ContainersFocus::ClusterList { BLUE } else { FG4 };
    let div_lines: Vec<Line> = (0..div_area.height)
        .map(|_| Line::from(Span::styled("│", Style::default().fg(div_color))))
        .collect();
    f.render_widget(Paragraph::new(div_lines), div_area);

    // ── Left: cluster list ────────────────────────────────────────────
    let left_rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(3)])
        .split(left_area);

    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(format!("  {} EKS Clusters", ICON_CUBE), Style::default().fg(BLUE).add_modifier(Modifier::BOLD)),
            if cs.loading_eks_clusters {
                Span::styled(format!("  {} loading…", app.spinner()), Style::default().fg(AMBER))
            } else {
                Span::styled(format!("  {}", cs.eks_clusters.len()), Style::default().fg(FG3))
            },
        ])),
        left_rows[0],
    );

    let cluster_focused = cs.focus == ContainersFocus::ClusterList;
    let mut items: Vec<ListItem> = Vec::new();

    if cs.eks_clusters.is_empty() && !cs.loading_eks_clusters {
        items.push(ListItem::new(Line::from(Span::styled(
            "  no clusters  (press r)",
            Style::default().fg(FG4),
        ))));
    } else {
        for (i, cluster) in cs.eks_clusters.iter().enumerate() {
            let selected = i == cs.selected_eks_cluster;
            let sel_bar = if selected && cluster_focused {
                Span::styled("▌", Style::default().fg(BLUE))
            } else {
                Span::styled(" ", Style::default())
            };
            let status_color = status_color(&cluster.status);
            let name_style = if selected && cluster_focused {
                Style::default().fg(FG).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(FG2)
            };
            let mut item = ListItem::new(Line::from(vec![
                sel_bar,
                Span::styled(format!(" {} ", ICON_CUBE), Style::default().fg(BLUE)),
                Span::styled(cluster.name.clone(), name_style),
                Span::styled(
                    format!("  v{}", cluster.version),
                    Style::default().fg(TEAL),
                ),
                Span::styled(
                    format!("  {}", cluster.status),
                    Style::default().fg(status_color),
                ),
            ]));
            if selected && cluster_focused {
                item = item.style(Style::default().bg(BG_HL));
            }
            items.push(item);
        }
    }
    f.render_widget(List::new(items), left_rows[1]);

    // ── Right: nodegroup list ──────────────────────────────────────────
    let right_rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(3)])
        .split(right_area);

    let ng_header = if !cs.eks_nodegroups_for.is_empty() {
        format!("  {} Node Groups — {}", ICON_NODES, cs.eks_nodegroups_for)
    } else {
        format!("  {} Node Groups", ICON_NODES)
    };
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(ng_header, Style::default().fg(MAUVE).add_modifier(Modifier::BOLD)),
            if cs.loading_eks_nodegroups {
                Span::styled(format!("  {} loading…", app.spinner()), Style::default().fg(AMBER))
            } else if !cs.eks_nodegroups.is_empty() {
                Span::styled(format!("  {}", cs.eks_nodegroups.len()), Style::default().fg(FG3))
            } else {
                Span::raw("")
            },
        ])),
        right_rows[0],
    );

    let detail_focused = cs.focus == ContainersFocus::DetailList;
    let mut ng_items: Vec<ListItem> = Vec::new();

    if let Some(ref err) = cs.last_error {
        ng_items.push(ListItem::new(Line::from(Span::styled(
            format!("  ✗ {}", err),
            Style::default().fg(RED),
        ))));
    } else if cs.eks_nodegroups.is_empty() && !cs.loading_eks_nodegroups {
        let hint = if cs.eks_nodegroups_for.is_empty() {
            "  Select a cluster and press Enter"
        } else {
            "  No node groups in this cluster"
        };
        ng_items.push(ListItem::new(Line::from(Span::styled(hint, Style::default().fg(FG4)))));
    } else {
        for (i, ng) in cs.eks_nodegroups.iter().enumerate() {
            let selected = i == cs.selected_eks_nodegroup;
            let sel_bar = if selected && detail_focused {
                Span::styled("▌", Style::default().fg(MAUVE))
            } else {
                Span::styled(" ", Style::default())
            };
            let status_color = status_color(&ng.status);
            let name_style = if selected && detail_focused {
                Style::default().fg(FG).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(FG2)
            };
            let mut item = ListItem::new(Line::from(vec![
                sel_bar,
                Span::styled(format!(" {} ", ICON_NODES), Style::default().fg(MAUVE)),
                Span::styled(ng.name.clone(), name_style),
                Span::styled(
                    format!("  {}/{}/{}", ng.desired, ng.min, ng.max),
                    Style::default().fg(GREEN),
                ),
                Span::styled(
                    format!("  {}", ng.status),
                    Style::default().fg(status_color),
                ),
                Span::styled(
                    format!("  {}", ng.instance_types),
                    Style::default().fg(FG4),
                ),
            ]));
            if selected && detail_focused {
                item = item.style(Style::default().bg(BG_HL));
            }
            ng_items.push(item);
        }
    }
    f.render_widget(List::new(ng_items), right_rows[1]);

    // Region dropdown
    if cs.region_dropdown_open {
        draw_region_dropdown(f, area, app);
    }
}

// ─── Region dropdown ──────────────────────────────────────────────────────────

fn draw_region_dropdown(f: &mut Frame, area: Rect, app: &App) {
    let cs = &app.containers_state;
    let dropdown_h = cs.regions.len().min(12) as u16 + 2;
    let dropdown_w = 18u16;
    let dx = area.x + 2;
    let dy = area.y + 3;
    let dropdown_area = Rect::new(dx, dy, dropdown_w, dropdown_h);

    f.render_widget(Clear, dropdown_area);

    let items: Vec<ListItem> = cs.regions.iter().enumerate().map(|(i, r)| {
        ListItem::new(Span::styled(
            format!(" {} ", r),
            if i == cs.region_idx {
                Style::default().fg(BG).bg(TEAL).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(FG2)
            },
        ))
    }).collect();

    f.render_widget(
        List::new(items).block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(TEAL))
                .style(Style::default().bg(BG_BAR)),
        ),
        dropdown_area,
    );
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn status_color(status: &str) -> Color {
    match status.to_uppercase().as_str() {
        "ACTIVE" | "RUNNING" | "HEALTHY" => GREEN,
        "DRAINING" | "PENDING" | "UPDATING" | "CREATING" => AMBER,
        "INACTIVE" | "FAILED" | "DELETE_FAILED" | "CREATE_FAILED" => RED,
        _ => FG3,
    }
}
