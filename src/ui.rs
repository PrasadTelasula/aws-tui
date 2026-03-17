use crate::app::*;
use crate::parser::AliasKind;
use crate::session::SessionStatus;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, BorderType, Borders, Clear, List, ListItem, Paragraph, Scrollbar,
        ScrollbarOrientation, ScrollbarState, Wrap,
    },
    Frame,
};

// ─── Catppuccin Macchiato palette ───────────────────────────────────
// Warm, slightly purple-tinted dark — not cold navy, not flat gray.
const BG: Color = Color::Rgb(36, 39, 58);             // #24273a base
const BG_ALT: Color = Color::Rgb(40, 44, 64);         // slightly lighter for alt rows
const BG_HL: Color = Color::Rgb(49, 50, 68);          // #313244 selection
const BG_BAR: Color = Color::Rgb(30, 32, 48);         // #1e2030 mantle

const FG: Color = Color::Rgb(202, 211, 245);           // #cad3f5 text
const FG2: Color = Color::Rgb(147, 154, 183);          // #939ab7 subtext
const FG3: Color = Color::Rgb(91, 96, 120);            // #5b6078 overlay
const FG4: Color = Color::Rgb(54, 58, 79);             // #363a4f surface border

const BLUE: Color = Color::Rgb(138, 173, 244);         // #8aadf4 sapphire
const GREEN: Color = Color::Rgb(166, 218, 149);        // #a6da95 green
const RED: Color = Color::Rgb(237, 135, 150);          // #ed8796 red
const AMBER: Color = Color::Rgb(238, 212, 159);        // #eed49f yellow
const TEAL: Color = Color::Rgb(139, 213, 202);         // #8bd5ca teal
const MAUVE: Color = Color::Rgb(198, 160, 246);        // #c6a0f6 mauve

// ─── Nerd Font icons (FA range, single-width, monochrome) ───────────
const ICON_CLOUD: &str = "\u{f0c2}";    //  cloud (AWS)
const ICON_KEY: &str = "\u{f084}";       //  key (SSO)
const ICON_PLUG: &str = "\u{f1e6}";      //  plug (SSM)
const ICON_SERVER: &str = "\u{f233}";    //  server (host)
const ICON_COG: &str = "\u{f013}";       //  cog (PID/process)
const ICON_CLOCK: &str = "\u{f017}";     //  clock (uptime)
const ICON_SEARCH: &str = "\u{f002}";    //  search
const ICON_FOLDER: &str = "\u{f07b}";    //  folder (group)
const ICON_TERM: &str = "\u{f120}";      //  terminal (command)
const ICON_GLOBE: &str = "\u{f0ac}";     //  globe (network/port)
const ICON_HASH: &str = "\u{f292}";      //  hashtag (target)

// ════════════════════════════════════════════════════════════════════

pub fn draw(f: &mut Frame, app: &App) {
    let size = f.area();
    f.render_widget(Block::default().style(Style::default().bg(BG)), size);

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Length(1),
            Constraint::Min(8),
            Constraint::Length(1),
        ])
        .split(size);

    draw_header(f, layout[0], app);
    draw_separator(f, layout[1]);
    draw_body(f, layout[2], app);
    draw_footer(f, layout[3], app);

    if app.input_mode == InputMode::Search {
        draw_search(f, size, app);
    }
    if app.show_confirm {
        draw_confirm(f, size, app);
    }
    if let Some(ref toast) = app.toast {
        draw_toast(f, size, toast);
    }
}

// ─── HEADER ─────────────────────────────────────────────────────────

fn draw_header(f: &mut Frame, area: Rect, app: &App) {
    f.render_widget(Block::default().style(Style::default().bg(BG_BAR)), area);

    let r1 = Rect::new(area.x + 2, area.y, area.width.saturating_sub(4), 1);
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(format!("{} ", ICON_CLOUD), Style::default().fg(TEAL)),
            Span::styled("AWS SSM", Style::default().fg(FG).add_modifier(Modifier::BOLD)),
            Span::styled("  session manager", Style::default().fg(FG3)),
        ])),
        r1,
    );

    let r2 = Rect::new(area.x + 2, area.y + 1, area.width.saturating_sub(4), 1);
    let active = if app.running_count > 0 {
        Span::styled(
            format!("{} {} active", app.spinner(), app.running_count),
            Style::default().fg(GREEN),
        )
    } else {
        Span::styled("idle", Style::default().fg(FG4))
    };

    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(format!("{} sessions", app.aliases.len()), Style::default().fg(FG3)),
            Span::styled("  ·  ", Style::default().fg(FG4)),
            active,
            Span::styled("  ·  ", Style::default().fg(FG4)),
            Span::styled(format!("{} {}", ICON_CLOCK, app.uptime_str()), Style::default().fg(FG3)),
        ])),
        r2,
    );
}

fn draw_separator(f: &mut Frame, area: Rect) {
    let line: String = "─".repeat(area.width as usize);
    f.render_widget(Paragraph::new(Span::styled(line, Style::default().fg(FG4))), area);
}

// ─── BODY ───────────────────────────────────────────────────────────

fn draw_body(f: &mut Frame, area: Rect, app: &App) {
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

    let left_area = Rect::new(inner.x, inner.y, left_w, inner.height);
    let div_area = Rect::new(inner.x + left_w, inner.y, 1, inner.height);
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
    let mut row_num: usize = 0;

    for &idx in &indices {
        let alias = &app.aliases[idx];
        let gk = group_key(alias);

        if gk != prev_gk {
            prev_gk = gk.clone();
            if !items.is_empty() {
                items.push(ListItem::new(Line::from("")));
                row_num += 1;
            }

            let (icon, kind_tag) = match &alias.kind {
                AliasKind::SsoLogin { .. }  => (ICON_KEY, "sso"),
                AliasKind::SsmSession { .. } => (ICON_PLUG, "ssm"),
                AliasKind::Other             => (ICON_TERM, "other"),
            };

            items.push(ListItem::new(Line::from(vec![
                Span::styled(format!("  {} ", icon), Style::default().fg(MAUVE)),
                Span::styled(
                    alias.group.clone(),
                    Style::default().fg(FG).add_modifier(Modifier::BOLD),
                ),
                Span::styled(format!("  {}", kind_tag), Style::default().fg(FG4)),
            ])));
            row_num += 1;
        }

        let is_last = group_last.get(&gk).copied() == Some(idx);
        let tree = if is_last { "  └ " } else { "  ├ " };

        let status = &app.session_statuses[idx];
        let selected = app.selected_index == idx;

        let (dot, dot_c) = match status {
            SessionStatus::Stopped  => ("·", FG4),
            SessionStatus::Starting => (app.spinner(), AMBER),
            SessionStatus::Running  => ("●", GREEN),
            SessionStatus::Error(_) => ("×", RED),
        };

        let name_style = if selected {
            Style::default().fg(FG).add_modifier(Modifier::BOLD)
        } else if matches!(status, SessionStatus::Running) {
            Style::default().fg(GREEN)
        } else {
            Style::default().fg(FG2)
        };

        let sel_bar = if selected {
            let c = match status {
                SessionStatus::Running  => GREEN,
                SessionStatus::Starting => AMBER,
                SessionStatus::Error(_) => RED,
                SessionStatus::Stopped  => BLUE,
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

        if let AliasKind::SsmSession { local_port: Some(p), .. } = &alias.kind {
            spans.push(Span::styled(format!(" :{}", p), Style::default().fg(TEAL)));
        }

        if matches!(status, SessionStatus::Running) {
            if let Some(u) = app.session_uptime(&alias.name) {
                spans.push(Span::styled(format!("  {}", u), Style::default().fg(FG3)));
            }
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

fn group_key(a: &crate::parser::Alias) -> String {
    let k = match &a.kind {
        AliasKind::SsoLogin { .. } => "sso",
        AliasKind::SsmSession { .. } => "ssm",
        AliasKind::Other => "other",
    };
    format!("{}:{}", a.group, k)
}

// ─── RIGHT: DETAIL + OUTPUT ─────────────────────────────────────────

fn draw_right(f: &mut Frame, area: Rect, app: &App) {
    if app.aliases.is_empty() {
        return;
    }

    let a = &app.aliases[app.selected_index];
    let st = &app.session_statuses[app.selected_index];

    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(8),
            Constraint::Length(1),
            Constraint::Min(2),
        ])
        .split(area);

    // ── Name + status ──
    let (status_text, status_c) = match st {
        SessionStatus::Stopped  => ("stopped",    FG3),
        SessionStatus::Starting => ("starting…",  AMBER),
        SessionStatus::Running  => ("running",    GREEN),
        SessionStatus::Error(e) => (e.as_str(),   RED),
    };

    let status_dot = match st {
        SessionStatus::Stopped  => Span::styled("· ", Style::default().fg(FG4)),
        SessionStatus::Starting => Span::styled(format!("{} ", app.spinner()), Style::default().fg(AMBER)),
        SessionStatus::Running  => Span::styled("● ", Style::default().fg(GREEN)),
        SessionStatus::Error(_) => Span::styled("× ", Style::default().fg(RED)),
    };

    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(" ", Style::default()),
            status_dot,
            Span::styled(&a.name, Style::default().fg(FG).add_modifier(Modifier::BOLD)),
            Span::styled("  ", Style::default()),
            Span::styled(status_text, Style::default().fg(status_c)),
        ])),
        sections[0],
    );

    thin_rule(f, sections[1]);

    // ── Details ──
    let pad = Rect::new(
        sections[2].x + 1,
        sections[2].y,
        sections[2].width.saturating_sub(2),
        sections[2].height,
    );

    let mut lines: Vec<Line> = Vec::new();

    lines.push(kv(ICON_FOLDER, "Group", vec![
        Span::styled(&a.group, Style::default().fg(MAUVE)),
    ]));

    match &a.kind {
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
            lines.push(kv(ICON_HASH, "Target", vec![
                Span::styled(target.as_str(), Style::default().fg(FG3)),
            ]));
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

    // ── Output header ──
    let oh = sections[3];
    let rule_w = oh.width.saturating_sub(11).max(1) as usize;
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(format!(" {} output ", ICON_TERM), Style::default().fg(FG3)),
            Span::styled("─".repeat(rule_w), Style::default().fg(FG4)),
        ])),
        oh,
    );

    // ── Output ──
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
        Span::styled(format!("{:<10}", label), Style::default().fg(FG3)),
    ];
    v.extend(values);
    Line::from(v)
}

fn thin_rule(f: &mut Frame, area: Rect) {
    let w = area.width.saturating_sub(2) as usize;
    f.render_widget(
        Paragraph::new(Span::styled(format!(" {}", "─".repeat(w)), Style::default().fg(FG4))),
        area,
    );
}

// ─── FOOTER ─────────────────────────────────────────────────────────

fn draw_footer(f: &mut Frame, area: Rect, app: &App) {
    let selected_status = if !app.aliases.is_empty() {
        &app.session_statuses[app.selected_index]
    } else {
        &SessionStatus::Stopped
    };

    let mut spans: Vec<Span> = vec![Span::styled(" ", Style::default())];

    spans.push(key_span("↑↓"));
    spans.push(desc_span(" navigate  "));

    match selected_status {
        SessionStatus::Running | SessionStatus::Starting => {
            spans.push(key_span("s"));
            spans.push(desc_span(" stop  "));
        }
        _ => {
            spans.push(key_span("Enter"));
            spans.push(desc_span(" start  "));
        }
    }

    if app.running_count > 0 {
        spans.push(key_span("S"));
        spans.push(desc_span(" stop all  "));
    }

    spans.push(key_span("Tab"));
    spans.push(desc_span(" switch  "));
    spans.push(key_span("/"));
    spans.push(desc_span(" search  "));
    spans.push(key_span("q"));
    spans.push(desc_span(" quit"));

    f.render_widget(
        Paragraph::new(Line::from(spans)).style(Style::default().bg(BG_BAR)),
        area,
    );
}

fn key_span(k: &str) -> Span<'_> {
    Span::styled(k, Style::default().fg(FG2))
}

fn desc_span(d: &str) -> Span<'_> {
    Span::styled(d, Style::default().fg(FG4))
}

// ─── SEARCH ─────────────────────────────────────────────────────────

fn draw_search(f: &mut Frame, area: Rect, app: &App) {
    if app.active_panel == ActivePanel::AliasList {
        return;
    }

    let w = 44u16.min(area.width.saturating_sub(6));
    let x = (area.width.saturating_sub(w)) / 2;
    let y = area.height / 4;
    let r = Rect::new(x, y, w, 3);

    dim_bg(f, area);
    f.render_widget(Clear, r);

    let cursor = if app.cursor_visible { "▏" } else { " " };
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(format!(" {} ", ICON_SEARCH), Style::default().fg(BLUE)),
            Span::styled(&app.search_query, Style::default().fg(FG)),
            Span::styled(cursor, Style::default().fg(BLUE)),
        ]))
        .block(popup_block("search", BLUE)),
        r,
    );
}

// ─── CONFIRM ────────────────────────────────────────────────────────

fn draw_confirm(f: &mut Frame, area: Rect, app: &App) {
    let w = 46u16.min(area.width.saturating_sub(6));
    let x = (area.width.saturating_sub(w)) / 2;
    let y = area.height / 3;
    let r = Rect::new(x, y, w, 5);

    dim_bg(f, area);
    f.render_widget(Clear, r);

    f.render_widget(
        Paragraph::new(vec![
            Line::from(""),
            Line::from(Span::styled(format!("  {}", &app.confirm_message), Style::default().fg(FG))),
            Line::from(vec![
                Span::styled("  y", Style::default().fg(GREEN).add_modifier(Modifier::BOLD)),
                Span::styled(" yes    ", Style::default().fg(FG3)),
                Span::styled("n", Style::default().fg(RED).add_modifier(Modifier::BOLD)),
                Span::styled(" no", Style::default().fg(FG3)),
            ]),
        ])
        .block(popup_block("confirm", FG3)),
        r,
    );
}

// ─── TOAST ──────────────────────────────────────────────────────────

fn draw_toast(f: &mut Frame, area: Rect, toast: &Toast) {
    let w = (toast.message.len() as u16 + 6).min(area.width.saturating_sub(4));
    let x = area.width.saturating_sub(w + 2);
    let r = Rect::new(x, 0, w, 3);
    f.render_widget(Clear, r);

    let c = match toast.kind {
        ToastKind::Success => GREEN,
        ToastKind::Error   => RED,
        ToastKind::Info    => BLUE,
    };

    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            format!("  {}", &toast.message),
            Style::default().fg(c),
        )))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(c))
                .style(Style::default().bg(BG_BAR)),
        ),
        r,
    );
}

// ─── Helpers ────────────────────────────────────────────────────────

fn popup_block(title: &str, color: Color) -> Block<'_> {
    Block::default()
        .title(Span::styled(format!(" {} ", title), Style::default().fg(color)))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(color))
        .style(Style::default().bg(BG_BAR))
}

fn dim_bg(f: &mut Frame, area: Rect) {
    f.render_widget(
        Block::default().style(
            Style::default()
                .bg(Color::Rgb(20, 22, 34))
                .add_modifier(Modifier::DIM),
        ),
        area,
    );
}
