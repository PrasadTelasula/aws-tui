use super::*;
use crate::app::App;
use crate::containers::{ContainersFocus, ContainersSubTab, EcsTreeItemKind};

fn draw_region_row(f: &mut Frame, area: Rect, app: &App) {
    let cs = &app.containers_state;
    let region_active = cs.focus == ContainersFocus::RegionList;
    let spans = vec![
        Span::styled(
            if region_active { " ▸ " } else { "   " },
            Style::default().fg(BLUE),
        ),
        Span::styled(format!("{} ", ICON_GLOBE), Style::default().fg(TEAL)),
        Span::styled(
            cs.active_region(),
            Style::default()
                .fg(if region_active { TEAL } else { FG2 })
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            if cs.region_dropdown_open { " ▴" } else { " ▾" },
            Style::default().fg(FG3),
        ),
        Span::styled("  Enter to change", Style::default().fg(FG4)),
    ];
    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

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
    use crate::containers::ContainersFocus;
    let cs = &app.containers_state;
    let bar_focused = cs.focus == ContainersFocus::SubTabBar;

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
        Span::styled(
            if bar_focused { " ▸ " } else { "   " },
            Style::default().fg(BLUE),
        ),
        Span::styled(format!(" {} ECS ", ICON_LAYERS), ecs_style),
        Span::styled("  ", Style::default()),
        Span::styled(format!(" {} EKS ", ICON_CUBE), eks_style),
        if bar_focused {
            Span::styled("  ←/→ or Enter to switch", Style::default().fg(FG4))
        } else {
            Span::raw("")
        },
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

    let div_color = if matches!(cs.focus, ContainersFocus::RegionList | ContainersFocus::SubTabBar | ContainersFocus::ClusterList) { BLUE } else { FG4 };
    let div_lines: Vec<Line> = (0..div_area.height)
        .map(|_| Line::from(Span::styled("│", Style::default().fg(div_color))))
        .collect();
    f.render_widget(Paragraph::new(div_lines), div_area);

    // ── Left: region | header | search | tree ────────────────────────
    let left_rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // region
            Constraint::Length(1), // header
            Constraint::Length(1), // search bar
            Constraint::Min(3),    // tree
        ])
        .split(left_area);

    draw_region_row(f, left_rows[0], app);

    // Header
    let cluster_count = cs.ecs_tree.iter().filter(|i| i.depth == 0).count();
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(format!("  {} ECS Clusters", ICON_LAYERS), Style::default().fg(BLUE).add_modifier(Modifier::BOLD)),
            if cs.loading_ecs_clusters {
                Span::styled(format!("  {} loading…", app.spinner()), Style::default().fg(AMBER))
            } else {
                Span::styled(format!("  {}", cluster_count), Style::default().fg(FG3))
            },
        ])),
        left_rows[1],
    );

    // Search bar
    let search_spans = if cs.ecs_search_active {
        vec![
            Span::styled(" / ", Style::default().fg(BLUE).add_modifier(Modifier::BOLD)),
            Span::styled(cs.ecs_search_query.clone(), Style::default().fg(FG)),
            Span::styled("█", Style::default().fg(BLUE)),
        ]
    } else if !cs.ecs_search_query.is_empty() {
        let match_count = cs.ecs_filtered_indices.len();
        vec![
            Span::styled(" / ", Style::default().fg(TEAL)),
            Span::styled(cs.ecs_search_query.clone(), Style::default().fg(TEAL)),
            Span::styled(format!("  {} match{}  Esc clear", match_count, if match_count == 1 { "" } else { "es" }), Style::default().fg(FG4)),
        ]
    } else {
        vec![Span::styled("   / to search", Style::default().fg(FG3))]
    };
    f.render_widget(Paragraph::new(Line::from(search_spans)), left_rows[2]);

    // Tree list
    let tree_focused = cs.focus == ContainersFocus::ClusterList;
    let mut items: Vec<ListItem> = Vec::new();

    let searching = !cs.ecs_search_query.is_empty();

    if cs.ecs_tree.is_empty() && !cs.loading_ecs_clusters {
        items.push(ListItem::new(Line::from(Span::styled(
            "  no clusters  (press r)",
            Style::default().fg(FG3),
        ))));
    } else if searching && cs.ecs_filtered_indices.is_empty() {
        items.push(ListItem::new(Line::from(Span::styled(
            "  no matches",
            Style::default().fg(FG3),
        ))));
    } else {
        let visible: Vec<usize> = if searching {
            cs.ecs_filtered_indices.clone()
        } else {
            (0..cs.ecs_tree.len()).collect()
        };
        for i in visible {
            if let Some(item) = cs.ecs_tree.get(i) {
                let selected = i == cs.selected_ecs_tree && tree_focused;
                let highlight = searching && cs.ecs_search_query.len() > 0;
                items.push(build_ecs_tree_item(item, selected, highlight, &cs.ecs_search_query));
            }
        }
    }
    f.render_widget(List::new(items), left_rows[3]);

    // ── Right: detail panel ───────────────────────────────────────────
    draw_ecs_detail_panel(f, right_area, app);

    // Region dropdown
    if cs.region_dropdown_open {
        draw_region_dropdown(f, area, app);
    }

    // Info popup (i key)
    if cs.show_info_popup {
        if let Some(ref popup) = cs.info_popup {
            draw_info_popup(f, area, popup, TEAL, app.spinner());
        }
    }
}

fn build_ecs_tree_item(
    item: &crate::containers::EcsTreeItem,
    selected: bool,
    highlight_match: bool,
    query: &str,
) -> ListItem<'static> {
    let indent: &str = match item.depth {
        0 => "",
        1 => "  ",
        2 => "    ",
        _ => "      ",
    };

    // Selection bar color by depth
    let sel_color = match item.depth {
        0 => BLUE,
        1 => TEAL,
        2 => MAUVE,
        _ => GREEN,
    };

    let sel_bar = if selected {
        Span::styled("▌", Style::default().fg(sel_color))
    } else {
        Span::styled(" ", Style::default())
    };

    // Name style: bright always, highlight matches in teal, bold when selected
    let name_color = if highlight_match && item.name.to_lowercase().contains(&query.to_lowercase()) {
        TEAL
    } else {
        FG
    };
    let name_style = if selected {
        Style::default().fg(name_color).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(name_color)
    };

    let mut spans: Vec<Span> = vec![sel_bar, Span::raw(indent)];

    match item.kind {
        EcsTreeItemKind::Cluster => {
            let (arrow, arrow_color) = if item.loading {
                ("⟳ ", AMBER)
            } else if item.expanded {
                ("▼ ", FG2)
            } else {
                ("▶ ", FG3)
            };
            spans.push(Span::styled(arrow, Style::default().fg(arrow_color)));
            spans.push(Span::styled(format!("{} ", ICON_LAYERS), Style::default().fg(BLUE)));
            spans.push(Span::styled(item.name.clone(), name_style));
            if !item.extra.is_empty() {
                spans.push(Span::styled(format!("  {}", item.extra), Style::default().fg(FG2)));
            }
            if item.count_b > 0 {
                spans.push(Span::styled(format!("  {} pend", item.count_b), Style::default().fg(AMBER)));
            }
            spans.push(Span::styled(
                format!("  {}", item.status),
                Style::default().fg(status_color(&item.status)),
            ));
        }
        EcsTreeItemKind::Service => {
            let (arrow, arrow_color) = if item.loading {
                ("⟳ ", AMBER)
            } else if item.expanded {
                ("▼ ", FG2)
            } else {
                ("▶ ", FG3)
            };
            spans.push(Span::styled(arrow, Style::default().fg(arrow_color)));
            spans.push(Span::styled(format!("{} ", ICON_COG2), Style::default().fg(TEAL)));
            spans.push(Span::styled(item.name.clone(), name_style));
            let count_color = if item.count_a == item.count_b { GREEN } else { AMBER };
            spans.push(Span::styled(
                format!("  {}/{}", item.count_a, item.count_b),
                Style::default().fg(count_color),
            ));
            spans.push(Span::styled(
                format!("  {}", item.status),
                Style::default().fg(status_color(&item.status)),
            ));
        }
        EcsTreeItemKind::Task => {
            // Show short task ID (last 12 chars)
            let display_name = if item.name.len() > 12 {
                item.name[item.name.len() - 12..].to_string()
            } else {
                item.name.clone()
            };
            spans.push(Span::styled("  ", Style::default()));
            spans.push(Span::styled(format!("{} ", ICON_CUBE), Style::default().fg(MAUVE)));
            spans.push(Span::styled(display_name, name_style));
            if !item.launch_type.is_empty() {
                let lt_color = if item.launch_type == "FARGATE" { BLUE } else { MAUVE };
                spans.push(Span::styled(
                    format!("  {}", item.launch_type),
                    Style::default().fg(lt_color),
                ));
            }
            spans.push(Span::styled(
                format!("  {}", item.status),
                Style::default().fg(status_color(&item.status)),
            ));
        }
        EcsTreeItemKind::Container => {
            spans.push(Span::styled("└ ", Style::default().fg(FG3)));
            spans.push(Span::styled(item.name.clone(), name_style));
            spans.push(Span::styled(
                format!("  {}", item.status),
                Style::default().fg(status_color(&item.status)),
            ));
            if !item.extra.is_empty() {
                let img = item.extra.split('/').last().unwrap_or(&item.extra);
                let img = if img.len() > 25 { &img[..25] } else { img };
                spans.push(Span::styled(format!("  {}", img), Style::default().fg(FG2)));
            }
        }
    }

    let mut list_item = ListItem::new(Line::from(spans));
    if selected {
        list_item = list_item.style(Style::default().bg(BG_HL));
    }
    list_item
}

fn draw_ecs_detail_panel(f: &mut Frame, area: Rect, app: &App) {
    let cs = &app.containers_state;

    if let Some(ref err) = cs.last_error {
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                format!("  ✗ {}", err),
                Style::default().fg(RED),
            ))),
            area,
        );
        return;
    }

    let Some(item) = cs.ecs_tree.get(cs.selected_ecs_tree) else {
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                "  Press r to load clusters",
                Style::default().fg(FG3),
            ))),
            area,
        );
        return;
    };

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(3)])
        .split(area);

    // Header
    let (header_text, header_color) = match item.kind {
        EcsTreeItemKind::Cluster   => (format!("  {} Cluster", ICON_LAYERS), BLUE),
        EcsTreeItemKind::Service   => (format!("  {} Service", ICON_COG2),   TEAL),
        EcsTreeItemKind::Task      => (format!("    {} Task",  ICON_CUBE),    MAUVE),
        EcsTreeItemKind::Container => (format!("      Container"),            GREEN),
    };
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(header_text, Style::default().fg(header_color).add_modifier(Modifier::BOLD)),
            Span::styled(format!("  {}", item.name), Style::default().fg(FG)),
        ])),
        rows[0],
    );

    // Detail lines
    let mut lines: Vec<Line> = Vec::new();
    match item.kind {
        EcsTreeItemKind::Cluster => {
            lines.push(detail_line("Status", &item.status, status_color(&item.status)));
            if !item.extra.is_empty() {
                lines.push(detail_line("Info", &item.extra, FG2));
            }
            if item.count_b > 0 {
                lines.push(detail_line("Pending", &item.count_b.to_string(), AMBER));
            }
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "  Enter/→ expand services  ←/Esc collapse",
                Style::default().fg(FG3),
            )));
        }
        EcsTreeItemKind::Service => {
            lines.push(detail_line("Status", &item.status, status_color(&item.status)));
            let count_color = if item.count_a == item.count_b { GREEN } else { AMBER };
            lines.push(detail_line(
                "Running",
                &format!("{}/{}", item.count_a, item.count_b),
                count_color,
            ));
            if !item.extra.is_empty() {
                lines.push(detail_line("Task Def", &item.extra, FG));
            }
            lines.push(detail_line("Cluster", &item.cluster_name, FG2));
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "  Enter/→ expand tasks  ←/Esc collapse",
                Style::default().fg(FG3),
            )));
        }
        EcsTreeItemKind::Task => {
            lines.push(detail_line("Task ID", &item.name, FG));
            lines.push(detail_line("Status", &item.status, status_color(&item.status)));
            if !item.launch_type.is_empty() {
                let lt_color = if item.launch_type == "FARGATE" { BLUE } else { MAUVE };
                lines.push(detail_line("Launch Type", &item.launch_type, lt_color));
            }
            if !item.extra.is_empty() {
                lines.push(detail_line("Task Def", &item.extra, FG));
            }
            lines.push(detail_line("Cluster", &item.cluster_name, FG2));
            lines.push(detail_line("Service", &item.service_name, FG2));
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "  e exec into task  i info",
                Style::default().fg(FG3),
            )));
        }
        EcsTreeItemKind::Container => {
            lines.push(detail_line("Status", &item.status, status_color(&item.status)));
            if !item.extra.is_empty() {
                lines.push(detail_line("Image", &item.extra, FG));
            }
            lines.push(detail_line("Service", &item.service_name, FG2));
            lines.push(detail_line("Cluster", &item.cluster_name, FG2));
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "  e exec into container  i info",
                Style::default().fg(FG3),
            )));
        }
    }
    f.render_widget(Paragraph::new(lines), rows[1]);
}

fn detail_line(label: &str, value: &str, value_color: Color) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("  {:<12}", label), Style::default().fg(FG2)),
        Span::styled(value.to_string(), Style::default().fg(value_color)),
    ])
}

// ─── EKS body ────────────────────────────────────────────────────────────────

fn draw_eks_body(f: &mut Frame, area: Rect, app: &App) {
    let cs = &app.containers_state;

    let left_w    = (area.width * 36) / 100;
    let right_w   = area.width.saturating_sub(left_w + 1);
    let left_area = Rect::new(area.x,              area.y, left_w,  area.height);
    let div_area  = Rect::new(area.x + left_w,     area.y, 1,       area.height);
    let right_area= Rect::new(area.x + left_w + 1, area.y, right_w, area.height);

    let div_color = if matches!(cs.focus, ContainersFocus::RegionList | ContainersFocus::SubTabBar | ContainersFocus::ClusterList) { BLUE } else { FG4 };
    let div_lines: Vec<Line> = (0..div_area.height)
        .map(|_| Line::from(Span::styled("│", Style::default().fg(div_color))))
        .collect();
    f.render_widget(Paragraph::new(div_lines), div_area);

    // ── Left: region selector + cluster list ──────────────────────────
    let left_rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(1), Constraint::Min(3)])
        .split(left_area);

    draw_region_row(f, left_rows[0], app);

    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(format!("  {} EKS Clusters", ICON_CUBE), Style::default().fg(BLUE).add_modifier(Modifier::BOLD)),
            if cs.loading_eks_clusters {
                Span::styled(format!("  {} loading…", app.spinner()), Style::default().fg(AMBER))
            } else {
                Span::styled(format!("  {}", cs.eks_clusters.len()), Style::default().fg(FG3))
            },
        ])),
        left_rows[1],
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
    f.render_widget(List::new(items), left_rows[2]);

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
