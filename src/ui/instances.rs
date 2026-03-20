use super::*;
use crate::app::App;
use crate::instances::{InstanceFocus, SsmConnectionStatus};

pub(super) fn draw_instances(f: &mut Frame, area: Rect, app: &App) {
    let is = &app.instances_state;

    if is.profiles.is_empty() {
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

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(5)])
        .split(inner);

    // Profile bar
    let mut profile_spans: Vec<Span> = vec![
        Span::styled(format!(" {} ", ICON_KEY), Style::default().fg(FG3)),
    ];
    for (i, p) in is.profiles.iter().enumerate() {
        let selected = i == is.active_profile_idx;
        profile_spans.push(Span::styled(
            format!(" {} ", p),
            if selected {
                Style::default().fg(BG).bg(GREEN).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(FG3)
            },
        ));
        profile_spans.push(Span::styled(" ", Style::default()));
    }
    profile_spans.push(Span::styled(
        format!("    {} {}", ICON_GLOBE, is.active_region()),
        Style::default().fg(TEAL),
    ));
    profile_spans.push(Span::styled("    Ctrl+H/L profile  Tab focus  r refresh", Style::default().fg(FG4)));
    f.render_widget(
        Paragraph::new(Line::from(profile_spans)).style(Style::default().bg(BG_BAR)),
        rows[0],
    );

    let left_w    = (rows[1].width * 36) / 100;
    let right_w   = rows[1].width.saturating_sub(left_w + 1);
    let left_area = Rect::new(rows[1].x,              rows[1].y, left_w,  rows[1].height);
    let div_area  = Rect::new(rows[1].x + left_w,     rows[1].y, 1,       rows[1].height);
    let right_area= Rect::new(rows[1].x + left_w + 1, rows[1].y, right_w, rows[1].height);

    let div_lines: Vec<Line> = (0..div_area.height)
        .map(|_| Line::from(Span::styled("│", Style::default().fg(FG4))))
        .collect();
    f.render_widget(Paragraph::new(div_lines), div_area);

    let left_split = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(1), Constraint::Min(3)])
        .split(left_area);

    // Region selector
    let region_active = is.focus == InstanceFocus::RegionList;
    let region_spans = vec![
        Span::styled(
            if region_active { " ▸ " } else { "   " },
            Style::default().fg(BLUE),
        ),
        Span::styled(format!("{} ", ICON_GLOBE), Style::default().fg(TEAL)),
        Span::styled(
            is.active_region(),
            Style::default().fg(if region_active { TEAL } else { FG2 }).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            if is.region_dropdown_open { " ▴" } else { " ▾" },
            Style::default().fg(FG3),
        ),
        Span::styled("  Enter to change", Style::default().fg(FG4)),
    ];
    f.render_widget(Paragraph::new(Line::from(region_spans)), left_split[0]);

    // Search bar
    let search_spans = if is.search_active {
        vec![
            Span::styled(" / ", Style::default().fg(BLUE).add_modifier(Modifier::BOLD)),
            Span::styled(is.search_query.as_str(), Style::default().fg(FG)),
            Span::styled("█", Style::default().fg(BLUE)),
        ]
    } else if !is.search_query.is_empty() {
        vec![
            Span::styled(" / ", Style::default().fg(TEAL)),
            Span::styled(is.search_query.as_str(), Style::default().fg(TEAL)),
            Span::styled("  Esc to clear", Style::default().fg(FG4)),
        ]
    } else {
        vec![Span::styled("   / to search", Style::default().fg(FG4))]
    };
    f.render_widget(Paragraph::new(Line::from(search_spans)), left_split[1]);

    // Instance list
    let inst_active = is.focus == InstanceFocus::InstanceList;
    let mut items: Vec<ListItem> = Vec::new();

    if is.loading_instances {
        items.push(ListItem::new(Line::from(vec![
            Span::styled(format!("  {} loading…", app.spinner()), Style::default().fg(AMBER)),
        ])));
    } else if is.instances.is_empty() {
        items.push(ListItem::new(Line::from(Span::styled(
            "  no instances",
            Style::default().fg(FG4),
        ))));
    } else {
        let visible: Vec<usize> = if !is.filtered_instances.is_empty() {
            is.filtered_instances.clone()
        } else if !is.search_query.is_empty() {
            vec![]
        } else {
            (0..is.instances.len()).collect()
        };

        if visible.is_empty() && !is.search_query.is_empty() {
            items.push(ListItem::new(Line::from(Span::styled(
                "  no matches",
                Style::default().fg(FG4),
            ))));
        } else {
            for i in visible {
                let inst = &is.instances[i];
                let selected = i == is.selected_instance;
                let sel_bar = if selected && inst_active {
                    Span::styled("▌", Style::default().fg(BLUE))
                } else {
                    Span::styled(" ", Style::default())
                };

                let name_style = if selected {
                    Style::default().fg(FG).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(FG2)
                };

                let (os_icon, os_color) = if inst.platform == "windows" {
                    (ICON_WINDOWS, BLUE)
                } else {
                    (ICON_LINUX, TEAL)
                };
                let mut item = ListItem::new(Line::from(vec![
                    sel_bar,
                    Span::styled(format!(" {} ", os_icon), Style::default().fg(os_color)),
                    Span::styled(format!("{} ", inst.name), name_style),
                    Span::styled(inst.instance_id.as_str(), Style::default().fg(FG3)),
                    Span::styled(format!("  {}", inst.private_ip), Style::default().fg(FG4)),
                ]));
                if selected && inst_active {
                    item = item.style(Style::default().bg(BG_HL));
                }
                items.push(item);
            }
        }
    }
    f.render_widget(List::new(items), left_split[2]);

    // Right panel: SSM terminals
    let has_sessions = !is.ssm_sessions.is_empty();
    let right_split = if has_sessions {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Min(3), Constraint::Length(1)])
            .split(right_area)
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(3), Constraint::Length(1)])
            .split(right_area)
    };

    let (term_area, status_area) = if has_sessions {
        let tab_area    = right_split[0];
        let term_area   = right_split[1];
        let status_area = right_split[2];

        let mut spans: Vec<Span> = Vec::new();
        for (i, session) in is.ssm_sessions.iter().enumerate() {
            let active = i == is.active_session_idx;
            let name: String = session.instance_name.chars().take(14).collect();
            let label = format!(" {} ", name);
            let dot = if session.status == SsmConnectionStatus::Connected { "● " } else { "○ " };
            let dot_color = if session.status == SsmConnectionStatus::Connected { GREEN } else { FG3 };
            if active {
                spans.push(Span::styled(dot, Style::default().fg(dot_color).bg(BG_HL)));
                spans.push(Span::styled(label, Style::default().fg(FG).bg(BG_HL).add_modifier(Modifier::BOLD)));
            } else {
                spans.push(Span::styled(dot, Style::default().fg(dot_color).bg(BG_BAR)));
                spans.push(Span::styled(label, Style::default().fg(FG2).bg(BG_BAR)));
            }
            spans.push(Span::styled("│", Style::default().fg(FG4).bg(BG_BAR)));
        }
        f.render_widget(
            Paragraph::new(Line::from(spans)).style(Style::default().bg(BG_BAR)),
            tab_area,
        );
        (term_area, status_area)
    } else {
        (right_split[0], right_split[1])
    };

    // Terminal area
    if let Some(session) = is.active_session() {
        let screen = session.parser.screen();
        let (screen_rows, screen_cols) = screen.size();

        let mut lines: Vec<Line> = Vec::new();
        for row in 0..screen_rows {
            let mut spans: Vec<Span> = Vec::new();
            let mut cur_text = String::new();
            let mut cur_style = Style::default();

            for col in 0..screen_cols {
                let (ch, style) = if let Some(cell) = screen.cell(row, col) {
                    let content = cell.contents();
                    let ch = if content.is_empty() { " ".to_string() } else { content.to_string() };
                    let fg = vt100_color(cell.fgcolor());
                    let bg = vt100_color(cell.bgcolor());
                    let mut s = Style::default().fg(fg).bg(bg);
                    if cell.bold()      { s = s.add_modifier(Modifier::BOLD); }
                    if cell.italic()    { s = s.add_modifier(Modifier::ITALIC); }
                    if cell.underline() { s = s.add_modifier(Modifier::UNDERLINED); }
                    (ch, s)
                } else {
                    (" ".to_string(), Style::default())
                };

                if style == cur_style {
                    cur_text.push_str(&ch);
                } else {
                    if !cur_text.is_empty() {
                        spans.push(Span::styled(cur_text.clone(), cur_style));
                        cur_text.clear();
                    }
                    cur_text = ch;
                    cur_style = style;
                }
            }
            if !cur_text.is_empty() {
                spans.push(Span::styled(cur_text, cur_style));
            }
            lines.push(Line::from(spans));
        }

        f.render_widget(Paragraph::new(lines), term_area);

        let (crow, ccol) = screen.cursor_position();
        let cx = term_area.x + ccol as u16;
        let cy = term_area.y + crow as u16;
        if cx < term_area.x + term_area.width && cy < term_area.y + term_area.height {
            f.set_cursor_position((cx, cy));
        }
    } else if let Some(ref err) = is.last_error {
        let max_w = term_area.width.saturating_sub(4).min(72);
        let inner_w = max_w.saturating_sub(2);
        let line_count = err.chars().count() as u16 / inner_w.max(1) + 1;
        let box_h = (line_count + 2 + 1).min(term_area.height.saturating_sub(2));
        let bx = term_area.x + (term_area.width.saturating_sub(max_w)) / 2;
        let by = term_area.y + (term_area.height.saturating_sub(box_h)) / 2;
        let err_area = Rect::new(bx, by, max_w, box_h);
        f.render_widget(Clear, err_area);
        f.render_widget(
            Paragraph::new(Span::styled(err.as_str(), Style::default().fg(RED)))
                .wrap(Wrap { trim: false })
                .block(
                    Block::default()
                        .title(Span::styled(" ✕ Connection Error ", Style::default().fg(RED)))
                        .borders(Borders::ALL)
                        .border_type(BorderType::Rounded)
                        .border_style(Style::default().fg(RED))
                        .style(Style::default().bg(BG_BAR)),
                ),
            err_area,
        );
    } else {
        f.render_widget(
            Paragraph::new(Span::styled(
                "  Select an instance and press Enter to connect",
                Style::default().fg(FG3),
            )),
            term_area,
        );
    }

    // Status bar
    let status_label = if has_sessions {
        if is.ssm_sessions.len() > 1 {
            "  F4/F5: switch session  [/]: nav  Ctrl+D: close  Tab: focus".to_string()
        } else {
            "  Ctrl+D: close  Tab: focus  Enter: new session".to_string()
        }
    } else {
        "  Enter: connect  Tab: focus".to_string()
    };
    f.render_widget(
        Paragraph::new(Span::styled(status_label, Style::default().fg(FG3)))
            .style(Style::default().bg(BG_BAR)),
        status_area,
    );

    // Region dropdown popup
    if is.region_dropdown_open {
        let dropdown_h = is.regions.len().min(12) as u16 + 2;
        let dropdown_w = 18u16;
        let dx = left_split[0].x + 3;
        let dy = left_split[0].y + 1;
        let dropdown_area = Rect::new(dx, dy, dropdown_w, dropdown_h);

        f.render_widget(Clear, dropdown_area);

        let items: Vec<ListItem> = is.regions.iter().enumerate().map(|(i, r)| {
            let selected = i == is.region_idx;
            ListItem::new(Span::styled(
                format!(" {} ", r),
                if selected {
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

    if is.show_info_popup {
        draw_instance_info_popup(f, area, app);
    }
}

fn draw_instance_info_popup(f: &mut Frame, area: Rect, app: &App) {
    let is = &app.instances_state;
    let popup = match &is.info_popup {
        Some(p) => p,
        None => return,
    };
    draw_info_popup(f, area, popup, TEAL, app.spinner());
}

