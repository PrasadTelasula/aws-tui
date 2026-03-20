use super::*;
use crate::app::{ActivePanel, App, InputMode};
use crate::parser::{Alias, AliasKind};
use crate::session::SessionStatus;

pub(super) fn draw_body(f: &mut Frame, area: Rect, app: &App) {
    let active_left = app.active_panel == ActivePanel::AliasList;

    let outer = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(FG4))
        .style(Style::default().bg(BG));
    let inner = outer.inner(area);
    f.render_widget(outer, area);

    let left_w = (inner.width * 38) / 100;
    let right_w = inner.width.saturating_sub(left_w + 1);

    let left_area  = Rect::new(inner.x,              inner.y, left_w,  inner.height);
    let div_area   = Rect::new(inner.x + left_w,     inner.y, 1,       inner.height);
    let right_area = Rect::new(inner.x + left_w + 1, inner.y, right_w, inner.height);

    let div_color = if active_left { BLUE } else { FG4 };
    let div_lines: Vec<Line> = (0..div_area.height)
        .map(|_| Line::from(Span::styled("│", Style::default().fg(div_color))))
        .collect();
    f.render_widget(Paragraph::new(div_lines), div_area);

    draw_list(f, left_area, app);
    draw_right(f, right_area, app);
}

// ─── LEFT: SESSION LIST ─────────────────────────────────────────────

fn draw_list(f: &mut Frame, area: Rect, app: &App) {
    let list_area = if app.input_mode == InputMode::Search || !app.search_query.is_empty() {
        let search_r = Rect::new(area.x, area.y, area.width, 1);

        let cursor = if app.input_mode == InputMode::Search {
            if app.cursor_visible { "▏" } else { " " }
        } else {
            ""
        };

        let count_text = if !app.search_query.is_empty() {
            format!("  {}", app.filtered_indices.len())
        } else {
            String::new()
        };

        f.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(format!(" {} ", ICON_SEARCH), Style::default().fg(BLUE)),
                Span::styled(&app.search_query, Style::default().fg(FG).add_modifier(Modifier::BOLD)),
                Span::styled(cursor, Style::default().fg(BLUE)),
                Span::styled(count_text, Style::default().fg(FG3)),
            ])).style(Style::default().bg(BG_HL)),
            search_r,
        );

        Rect::new(area.x, area.y + 1, area.width, area.height.saturating_sub(1))
    } else {
        area
    };

    if app.filtered_indices.is_empty() && !app.search_query.is_empty() {
        f.render_widget(
            Paragraph::new(Span::styled("  no matches", Style::default().fg(FG3))),
            Rect::new(list_area.x, list_area.y + 1, list_area.width, 1),
        );
        return;
    }

    let indices: Vec<usize> = if app.search_query.is_empty() {
        (0..app.aliases.len()).collect()
    } else {
        app.filtered_indices.clone()
    };

    let mut group_last: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for &idx in indices.iter().rev() {
        let gk = group_key(&app.aliases[idx]);
        group_last.entry(gk).or_insert(idx);
    }

    let mut items: Vec<ListItem> = Vec::new();
    let mut prev_gk = String::new();
    let mut prev_group = String::new();
    let mut row_num: usize = 0;

    for &idx in &indices {
        let alias = &app.aliases[idx];
        let gk = group_key(alias);

        if gk != prev_gk {
            if alias.group != prev_group {
                if !items.is_empty() {
                    items.push(ListItem::new(Line::from("")));
                    row_num += 1;
                }
                items.push(ListItem::new(Line::from(vec![
                    Span::styled(format!("  {} ", ICON_FOLDER_OPEN), Style::default().fg(MAUVE)),
                    Span::styled(
                        alias.group.clone(),
                        Style::default().fg(FG).add_modifier(Modifier::BOLD),
                    ),
                ])));
                row_num += 1;
                prev_group = alias.group.clone();
            }

            let (sg_icon, sg_color) = match &alias.subgroup {
                Some(sg) => subgroup_icon(sg),
                None => {
                    match &alias.kind {
                        AliasKind::SsoLogin { .. }  => (ICON_SHIELD, BLUE),
                        AliasKind::SsmSession { .. } => (ICON_PLUG, TEAL),
                        AliasKind::IamProfile { .. } => (ICON_KEY, AMBER),
                        AliasKind::Other             => (ICON_TAG, FG2),
                    }
                }
            };
            let sg_label = alias.subgroup.clone().unwrap_or_else(|| {
                match &alias.kind {
                    AliasKind::SsoLogin { .. }  => "SSO".to_string(),
                    AliasKind::SsmSession { .. } => "SSM".to_string(),
                    AliasKind::IamProfile { .. } => "IAM".to_string(),
                    AliasKind::Other             => "Other".to_string(),
                }
            });

            items.push(ListItem::new(Line::from(vec![
                Span::styled(format!("    {} ", sg_icon), Style::default().fg(sg_color)),
                Span::styled(sg_label, Style::default().fg(sg_color).add_modifier(Modifier::BOLD)),
            ])));
            row_num += 1;
            prev_gk = gk.clone();
        }

        let is_last = group_last.get(&gk).copied() == Some(idx);
        let tree = if is_last { "      └ " } else { "      ├ " };

        let status = &app.session_statuses[idx];
        let selected = app.selected_index == idx;

        let (dot, dot_c) = match status {
            SessionStatus::Stopped   => ("·", FG4),
            SessionStatus::Starting  => (app.spinner(), AMBER),
            SessionStatus::Running   => ("●", GREEN),
            SessionStatus::Connected => ("●", TEAL),
            SessionStatus::Expired   => ("○", AMBER),
            SessionStatus::Error(_)  => ("×", RED),
        };

        let name_style = if selected {
            Style::default().fg(FG).add_modifier(Modifier::BOLD)
        } else if matches!(status, SessionStatus::Running | SessionStatus::Connected) {
            Style::default().fg(GREEN)
        } else {
            Style::default().fg(FG2)
        };

        let sel_bar = if selected {
            let c = match status {
                SessionStatus::Running   => GREEN,
                SessionStatus::Connected => TEAL,
                SessionStatus::Starting  => AMBER,
                SessionStatus::Expired   => AMBER,
                SessionStatus::Error(_)  => RED,
                SessionStatus::Stopped   => BLUE,
            };
            Span::styled("▌", Style::default().fg(c))
        } else {
            Span::styled(" ", Style::default())
        };

        let mut spans = vec![
            sel_bar,
            Span::styled(tree, Style::default().fg(FG4)),
            Span::styled(format!("{} ", dot), Style::default().fg(dot_c)),
            Span::styled(&alias.name, name_style),
        ];

        if let AliasKind::SsmSession { local_port, target, .. } = &alias.kind {
            if let Some(p) = local_port {
                spans.push(Span::styled(format!(" :{}", p), Style::default().fg(TEAL)));
            }
            if let Some(tag_str) = format_tag_target(target) {
                spans.push(Span::styled(
                    format!("  Tags: {}", tag_str),
                    Style::default().fg(FG3),
                ));
            }
        }

        if matches!(status, SessionStatus::Running) {
            if let Some(u) = app.session_uptime(&alias.name) {
                spans.push(Span::styled(format!("  {}", u), Style::default().fg(FG3)));
            }
        }

        if matches!(status, SessionStatus::Connected) {
            spans.push(Span::styled("  connected", Style::default().fg(TEAL)));
        }
        if matches!(status, SessionStatus::Expired) {
            spans.push(Span::styled("  expired", Style::default().fg(AMBER)));
        }

        let row_bg = if selected {
            BG_HL
        } else if row_num % 2 == 0 {
            BG
        } else {
            BG_ALT
        };

        items.push(ListItem::new(Line::from(spans)).style(Style::default().bg(row_bg)));
        row_num += 1;
    }

    f.render_widget(List::new(items), list_area);

    if indices.len() * 2 > list_area.height as usize {
        let mut sb = ScrollbarState::new(indices.len() * 2).position(app.list_scroll_offset);
        f.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight).style(Style::default().fg(FG4)),
            list_area,
            &mut sb,
        );
    }
}

fn group_key(a: &Alias) -> String {
    format!("{}:{}", a.group, a.subgroup.as_deref().unwrap_or(""))
}

fn format_tag_target(target: &str) -> Option<String> {
    let tag_part = target.strip_prefix("tag:")?;
    let formatted: Vec<String> = tag_part
        .split(',')
        .filter_map(|pair| {
            let mut it = pair.splitn(2, '=');
            let key = it.next()?.trim();
            let val = it.next()?.trim();
            Some(format!("{}:{}", key, val))
        })
        .collect();
    if formatted.is_empty() { None } else { Some(formatted.join(", ")) }
}

fn subgroup_icon(subgroup: &str) -> (&'static str, Color) {
    match subgroup.to_lowercase().as_str() {
        s if s.contains("sso") || s.contains("login") || s.contains("auth") => (ICON_SHIELD, BLUE),
        s if s.contains("db") || s.contains("database") || s.contains("rds") => (ICON_DATABASE, AMBER),
        s if s.contains("os") || s.contains("shell") || s.contains("host") => (ICON_DESKTOP, TEAL),
        s if s.contains("vpn") || s.contains("tunnel") || s.contains("net") => (ICON_NETWORK, MAUVE),
        _ => (ICON_TAG, FG2),
    }
}

// ─── RIGHT: DETAIL + OUTPUT ─────────────────────────────────────────

fn draw_right(f: &mut Frame, area: Rect, app: &App) {
    if app.aliases.is_empty() {
        return;
    }

    let a = &app.aliases[app.selected_index];
    let st = &app.session_statuses[app.selected_index];

    let has_creds = matches!(st, SessionStatus::Connected)
        && app.session_credentials.contains_key(&a.name);
    let details_height = if has_creds { 12 } else { 8 };

    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(details_height),
            Constraint::Length(1),
            Constraint::Min(2),
        ])
        .split(area);

    let (status_text, status_c) = match st {
        SessionStatus::Stopped   => ("stopped",   FG3),
        SessionStatus::Starting  => ("starting…", AMBER),
        SessionStatus::Running   => ("running",   GREEN),
        SessionStatus::Connected => ("connected", TEAL),
        SessionStatus::Expired   => ("expired",   AMBER),
        SessionStatus::Error(e)  => (e.as_str(),  RED),
    };

    let max_status_len = (sections[0].width as usize).saturating_sub(a.name.len() + 6);
    let truncated_status: String;
    let display_status = if status_text.len() > max_status_len && max_status_len > 3 {
        truncated_status = format!("{}…", &status_text[..max_status_len.saturating_sub(1)]);
        truncated_status.as_str()
    } else {
        status_text
    };

    let status_dot = match st {
        SessionStatus::Stopped   => Span::styled("· ", Style::default().fg(FG4)),
        SessionStatus::Starting  => Span::styled(format!("{} ", app.spinner()), Style::default().fg(AMBER)),
        SessionStatus::Running   => Span::styled("● ", Style::default().fg(GREEN)),
        SessionStatus::Connected => Span::styled("● ", Style::default().fg(TEAL)),
        SessionStatus::Expired   => Span::styled("○ ", Style::default().fg(AMBER)),
        SessionStatus::Error(_)  => Span::styled("× ", Style::default().fg(RED)),
    };

    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(" ", Style::default()),
            status_dot,
            Span::styled(&a.name, Style::default().fg(FG).add_modifier(Modifier::BOLD)),
            Span::styled("  ", Style::default()),
            Span::styled(display_status, Style::default().fg(status_c)),
        ])),
        sections[0],
    );

    thin_rule(f, sections[1]);

    let pad = Rect::new(
        sections[2].x + 1,
        sections[2].y,
        sections[2].width.saturating_sub(2),
        sections[2].height,
    );

    let mut lines: Vec<Line> = Vec::new();

    if let SessionStatus::Error(e) = st {
        let avail_w = pad.width.saturating_sub(2) as usize;
        let words = e.split_whitespace();
        let mut current = String::new();
        for word in words {
            if current.is_empty() {
                current.push_str(word);
            } else if current.len() + 1 + word.len() <= avail_w {
                current.push(' ');
                current.push_str(word);
            } else {
                lines.push(Line::from(Span::styled(
                    format!(" × {}", current),
                    Style::default().fg(RED),
                )));
                current = word.to_string();
            }
        }
        if !current.is_empty() {
            lines.push(Line::from(Span::styled(
                format!(" × {}", current),
                Style::default().fg(RED),
            )));
        }
        lines.push(Line::from(""));
    }

    lines.push(kv(ICON_FOLDER_OPEN, "Group", vec![
        Span::styled(&a.group, Style::default().fg(MAUVE)),
    ]));

    match &a.kind {
        AliasKind::IamProfile { profile_name } => {
            lines.push(kv(ICON_KEY, "Type", vec![
                Span::styled("IAM Profile", Style::default().fg(AMBER)),
            ]));
            lines.push(kv(ICON_KEY, "Profile", vec![
                Span::styled(profile_name.as_str(), Style::default().fg(AMBER)),
            ]));
        }
        AliasKind::SsoLogin { session_name } => {
            lines.push(kv(ICON_KEY, "Type", vec![
                Span::styled("SSO Login", Style::default().fg(FG2)),
            ]));
            lines.push(kv(ICON_KEY, "Session", vec![
                Span::styled(session_name.as_str(), Style::default().fg(AMBER)),
            ]));
        }
        AliasKind::SsmSession { target, local_port, remote_port, host, .. } => {
            lines.push(kv(ICON_PLUG, "Type", vec![
                Span::styled("SSM Port Forward", Style::default().fg(FG2)),
            ]));
            if let Some(tag_str) = format_tag_target(target) {
                lines.push(kv(ICON_TAG, "Tags", vec![
                    Span::styled(tag_str, Style::default().fg(AMBER)),
                ]));
            } else {
                lines.push(kv(ICON_HASH, "Target", vec![
                    Span::styled(target.as_str(), Style::default().fg(FG3)),
                ]));
            }
            match (local_port, remote_port) {
                (Some(l), Some(r)) => {
                    lines.push(kv(ICON_GLOBE, "Ports", vec![
                        Span::styled(l.as_str(), Style::default().fg(TEAL).add_modifier(Modifier::BOLD)),
                        Span::styled(" → ", Style::default().fg(FG4)),
                        Span::styled(r.as_str(), Style::default().fg(TEAL).add_modifier(Modifier::BOLD)),
                    ]));
                }
                (Some(l), None) => {
                    lines.push(kv(ICON_GLOBE, "Port", vec![
                        Span::styled(l.as_str(), Style::default().fg(TEAL).add_modifier(Modifier::BOLD)),
                    ]));
                }
                _ => {}
            }
            if let Some(h) = host {
                lines.push(kv(ICON_SERVER, "Host", vec![
                    Span::styled(h.as_str(), Style::default().fg(FG2)),
                ]));
            }
        }
        AliasKind::Other => {
            lines.push(kv(ICON_TERM, "Type", vec![
                Span::styled("Other", Style::default().fg(FG3)),
            ]));
        }
    }

    if let Some(pid) = app.session_pids.get(&app.selected_index).copied().flatten() {
        let mut v = vec![Span::styled(format!("{}", pid), Style::default().fg(FG2))];
        if let Some(u) = app.session_uptime(&a.name) {
            v.push(Span::styled(format!("  up {}", u), Style::default().fg(FG3)));
        }
        lines.push(kv(ICON_COG, "PID", v));
    }

    if let Some((info, _)) = app.token_expiry.get(&a.name) {
        lines.push(kv(ICON_GLOBE, "Identity", vec![
            Span::styled(info.as_str(), Style::default().fg(TEAL)),
        ]));
    }

    if let Some(c) = app.session_credentials.get(&a.name) {
        lines.push(kv(ICON_KEY, "AccessKeyId", vec![
            Span::styled(&c.access_key_id, Style::default().fg(BLUE)),
        ]));
        let secret_display = if c.secret_access_key.len() > 8 {
            format!("{}••••••••", &c.secret_access_key[..4])
        } else {
            "••••••••".to_string()
        };
        lines.push(kv(ICON_KEY, "SecretKey", vec![
            Span::styled(secret_display, Style::default().fg(FG2)),
        ]));
        if !c.session_token.is_empty() {
            let token_display = if c.session_token.len() > 28 {
                format!("{}…", &c.session_token[..28])
            } else {
                c.session_token.clone()
            };
            lines.push(kv(ICON_KEY, "Token", vec![
                Span::styled(token_display, Style::default().fg(FG3)),
            ]));
        }
        if !c.expiration.is_empty() {
            lines.push(kv(ICON_CLOCK, "Expiration", vec![
                Span::styled(&c.expiration, Style::default().fg(FG2)),
            ]));
        }
        lines.push(kv(ICON_CLOCK, "Local Time", vec![
            Span::styled(
                chrono::Local::now().format("%Y-%m-%dT%H:%M:%S").to_string(),
                Style::default().fg(FG2),
            ),
        ]));
    }

    if matches!(st, SessionStatus::Connected) {
        lines.push(kv(ICON_CLOCK, "Status", vec![
            Span::styled("verified", Style::default().fg(GREEN)),
            Span::styled("  (checked every 5m)", Style::default().fg(FG3)),
        ]));
    } else if matches!(st, SessionStatus::Expired) {
        lines.push(kv(ICON_CLOCK, "Status", vec![
            Span::styled("expired — re-login required", Style::default().fg(AMBER)),
        ]));
    }

    let max_cmd = (pad.width as usize).saturating_sub(16);
    let cmd_str = if a.command.len() > max_cmd && max_cmd > 3 {
        format!("{}…", &a.command[..max_cmd - 1])
    } else {
        a.command.clone()
    };
    lines.push(kv(ICON_TERM, "Cmd", vec![
        Span::styled(cmd_str, Style::default().fg(FG3)),
    ]));

    f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), pad);

    let oh = sections[3];
    let rule_w = oh.width.saturating_sub(11).max(1) as usize;
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(format!(" {} output ", ICON_TERM), Style::default().fg(FG3)),
            Span::styled("─".repeat(rule_w), Style::default().fg(FG4)),
        ])),
        oh,
    );

    let out_area = sections[4];
    let output = app.session_outputs.get(&a.name).cloned().unwrap_or_default();

    if output.is_empty() {
        let hint = match st {
            SessionStatus::Stopped  => " press Enter to start",
            SessionStatus::Starting => " connecting…",
            _                       => " waiting for output…",
        };
        f.render_widget(
            Paragraph::new(Span::styled(hint, Style::default().fg(FG4))),
            out_area,
        );
        return;
    }

    let lines: Vec<Line> = output
        .iter()
        .enumerate()
        .map(|(i, line)| {
            let n = Span::styled(format!(" {:>3} │ ", i + 1), Style::default().fg(FG4));
            let c = if line.starts_with(">>>") {
                BLUE
            } else if line.contains("[stderr]") {
                AMBER
            } else if line.contains("rror") {
                RED
            } else {
                FG2
            };
            Line::from(vec![n, Span::styled(line.as_str(), Style::default().fg(c))])
        })
        .collect();

    let total = lines.len();
    let vis = out_area.height as usize;
    let scroll = if total > vis { (total - vis) as u16 } else { 0 };

    f.render_widget(
        Paragraph::new(lines).wrap(Wrap { trim: false }).scroll((scroll, 0)),
        out_area,
    );

    if total > vis {
        let mut sb = ScrollbarState::new(total).position(scroll as usize);
        f.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight).style(Style::default().fg(FG4)),
            out_area,
            &mut sb,
        );
    }
}

fn kv<'a>(icon: &'a str, label: &'a str, values: Vec<Span<'a>>) -> Line<'a> {
    let mut v = vec![
        Span::styled(format!(" {} ", icon), Style::default().fg(FG3)),
        Span::styled(format!("{:<14}", label), Style::default().fg(FG3)),
    ];
    v.extend(values);
    Line::from(v)
}
