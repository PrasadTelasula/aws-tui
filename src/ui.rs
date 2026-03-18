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

fn vt100_color(c: vt100::Color) -> Color {
    match c {
        vt100::Color::Default => Color::Reset,
        vt100::Color::Idx(n) => Color::Indexed(n),
        vt100::Color::Rgb(r, g, b) => Color::Rgb(r, g, b),
    }
}

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
            Constraint::Length(1),  // tab bar
            Constraint::Length(1),  // separator
            Constraint::Min(8),
            Constraint::Length(1),
        ])
        .split(size);

    draw_header(f, layout[0], app);
    draw_tab_bar(f, layout[1], app);
    draw_separator(f, layout[2]);

    match app.active_tab {
        AppTab::Sessions  => draw_body(f, layout[3], app),
        AppTab::Terminal   => draw_terminal(f, layout[3], app),
        AppTab::Instances  => draw_instances(f, layout[3], app),
    }
    draw_footer(f, layout[4], app);

    if app.input_mode == InputMode::Search {
        draw_search(f, size, app);
    }
    if app.show_confirm {
        draw_confirm(f, size, app);
    }
    if app.show_credentials_popup {
        draw_credentials_popup(f, size, app);
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

fn draw_tab_bar(f: &mut Frame, area: Rect, app: &App) {
    let tab_style = |tab: AppTab| -> Style {
        if app.active_tab == tab {
            Style::default().fg(BLUE).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(FG3)
        }
    };
    let bar = |tab: AppTab| -> &str {
        if app.active_tab == tab { "▎" } else { " " }
    };

    let line = Line::from(vec![
        Span::styled(format!(" {}", bar(AppTab::Sessions)), Style::default().fg(BLUE)),
        Span::styled(format!(" {} Sessions ", ICON_PLUG), tab_style(AppTab::Sessions)),
        Span::styled("  ", Style::default()),
        Span::styled(bar(AppTab::Terminal), Style::default().fg(BLUE)),
        Span::styled(format!(" {} Terminal ", ICON_TERM), tab_style(AppTab::Terminal)),
        Span::styled("  ", Style::default()),
        Span::styled(bar(AppTab::Instances), Style::default().fg(BLUE)),
        Span::styled(format!(" {} Instances ", ICON_SERVER), tab_style(AppTab::Instances)),
        Span::styled("    ", Style::default()),
        Span::styled("F1/F2/F3", Style::default().fg(FG4)),
    ]);

    f.render_widget(
        Paragraph::new(line).style(Style::default().bg(BG_BAR)),
        area,
    );
}

fn draw_separator(f: &mut Frame, area: Rect) {
    let line: String = "─".repeat(area.width as usize);
    f.render_widget(Paragraph::new(Span::styled(line, Style::default().fg(FG4))), area);
}

// ─── TERMINAL TAB ───────────────────────────────────────────────────

fn draw_terminal(f: &mut Frame, area: Rect, app: &App) {
    let ts = &app.terminal_state;

    let outer = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(FG4))
        .style(Style::default().bg(BG));
    let inner = outer.inner(area);
    f.render_widget(outer, area);

    let has_tabs = ts.terminals.len() > 1;

    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints(if has_tabs {
            vec![
                Constraint::Length(1),  // terminal tabs
                Constraint::Min(3),    // output
                Constraint::Length(1), // separator
                Constraint::Length(1), // input
            ]
        } else {
            vec![
                Constraint::Length(0),
                Constraint::Min(3),
                Constraint::Length(1),
                Constraint::Length(1),
            ]
        })
        .split(inner);

    if has_tabs {
        draw_terminal_tabs(f, sections[0], ts);
    }

    let term = ts.active();
    draw_terminal_output(f, sections[1], term, app);
    thin_rule(f, sections[2]);
    draw_terminal_input(f, sections[3], term, app);

    if term.completer.visible {
        draw_suggestions(f, sections[3], inner, term);
    }
}

fn draw_terminal_tabs(f: &mut Frame, area: Rect, ts: &crate::terminal::TerminalState) {
    let mut spans: Vec<Span> = vec![
        Span::styled(format!(" {} ", ICON_TERM), Style::default().fg(FG3)),
    ];

    for (i, term) in ts.terminals.iter().enumerate() {
        let is_active = i == ts.active_idx;
        let label = term.profile_label();

        // Show running command count
        let running = term.entries.iter().filter(|e| e.is_running).count();
        let badge = if running > 0 {
            format!(" {} ({}) ", label, running)
        } else {
            format!(" {} ", label)
        };

        spans.push(Span::styled(
            badge,
            if is_active {
                Style::default().fg(BG).bg(BLUE).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(FG2)
            },
        ));
        if i < ts.terminals.len() - 1 {
            spans.push(Span::styled(" ", Style::default()));
        }
    }

    spans.push(Span::styled("    ", Style::default()));
    spans.push(Span::styled("Ctrl+H/L switch", Style::default().fg(FG4)));

    f.render_widget(
        Paragraph::new(Line::from(spans)).style(Style::default().bg(BG_BAR)),
        area,
    );
}

fn draw_terminal_output(f: &mut Frame, area: Rect, term: &crate::terminal::TerminalInstance, app: &App) {
    if term.entries.is_empty() {
        let profile_hint = if let Some(ref p) = term.profile {
            format!("  Profile: {}  ·  Commands run with AWS_PROFILE={}", p, p)
        } else {
            "  Default terminal  ·  No profile set".to_string()
        };

        let hints = vec![
            Line::from(""),
            Line::from(Span::styled(
                "  Type an AWS CLI command and press Enter",
                Style::default().fg(FG3),
            )),
            Line::from(Span::styled(profile_hint, Style::default().fg(FG4))),
            Line::from(Span::styled(
                "  Tab to accept suggestion  ·  Up/Down for history  ·  PgUp/PgDn to scroll",
                Style::default().fg(FG4),
            )),
        ];
        f.render_widget(Paragraph::new(hints), area);
        return;
    }

    let mut lines: Vec<Line> = Vec::new();

    for entry in &term.entries {
        let status_icon = if entry.is_running {
            Span::styled(format!("{} ", app.spinner()), Style::default().fg(AMBER))
        } else {
            match entry.exit_code {
                Some(0) => Span::styled("✓ ", Style::default().fg(GREEN)),
                _       => Span::styled("✗ ", Style::default().fg(RED)),
            }
        };

        lines.push(Line::from(vec![
            Span::styled(" ", Style::default()),
            status_icon,
            Span::styled("$ ", Style::default().fg(BLUE)),
            Span::styled(
                entry.command.as_str(),
                Style::default().fg(FG).add_modifier(Modifier::BOLD),
            ),
        ]));

        for line in &entry.output_lines {
            let color = if line.starts_with("[stderr]") { AMBER }
                else if line.to_lowercase().contains("error") { RED }
                else { FG2 };
            lines.push(Line::from(vec![
                Span::styled("   ", Style::default()),
                Span::styled(line.as_str(), Style::default().fg(color)),
            ]));
        }

        lines.push(Line::from(""));
    }

    let total = lines.len();
    let visible = area.height as usize;
    let bottom_scroll = if total > visible { total - visible } else { 0 };
    let scroll = bottom_scroll.saturating_sub(term.scroll_offset);

    f.render_widget(
        Paragraph::new(lines).scroll((scroll as u16, 0)),
        area,
    );

    if total > visible {
        let mut sb = ScrollbarState::new(total).position(scroll);
        f.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .style(Style::default().fg(FG4)),
            area,
            &mut sb,
        );
    }
}

fn draw_terminal_input(f: &mut Frame, area: Rect, term: &crate::terminal::TerminalInstance, app: &App) {
    let cursor = if app.input_mode == InputMode::TerminalInput && app.cursor_visible {
        "▏"
    } else {
        ""
    };

    let (before, after) = if term.cursor_pos <= term.input.len() {
        term.input.split_at(term.cursor_pos)
    } else {
        (term.input.as_str(), "")
    };

    let bg = if app.input_mode == InputMode::TerminalInput { BG_HL } else { BG };

    let profile_span = if let Some(ref prof) = term.profile {
        Span::styled(format!(" [{}] ", prof), Style::default().fg(GREEN))
    } else {
        Span::styled(format!(" {} ", ICON_CLOUD), Style::default().fg(TEAL))
    };

    f.render_widget(
        Paragraph::new(Line::from(vec![
            profile_span,
            Span::styled("$ ", Style::default().fg(BLUE)),
            Span::styled(before, Style::default().fg(FG)),
            Span::styled(cursor, Style::default().fg(BLUE)),
            Span::styled(after, Style::default().fg(FG)),
        ])).style(Style::default().bg(bg)),
        area,
    );
}

fn draw_suggestions(f: &mut Frame, input_area: Rect, container: Rect, term: &crate::terminal::TerminalInstance) {
    let result = match &term.completer.cached_result {
        Some(r) if !r.suggestions.is_empty() => r,
        _ => return,
    };

    let max_visible = 8;
    let count = result.suggestions.len().min(max_visible);
    let popup_h = count as u16 + 2;

    let max_w = result.suggestions.iter()
        .take(max_visible)
        .map(|s| s.len())
        .max()
        .unwrap_or(10);
    let popup_w = (max_w as u16 + 4).min(container.width.saturating_sub(4));

    let x_off = 5 + result.prefix_start as u16;
    let x = (container.x + x_off).min(container.x + container.width - popup_w);
    let y = input_area.y.saturating_sub(popup_h);

    let popup = Rect::new(x, y, popup_w, popup_h);
    f.render_widget(Clear, popup);

    let items: Vec<ListItem> = result.suggestions.iter()
        .take(max_visible)
        .enumerate()
        .map(|(i, s)| {
            let style = if i == term.completer.selected_index {
                Style::default().fg(FG).bg(BG_HL).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(FG2)
            };
            ListItem::new(Span::styled(format!(" {} ", s), style))
        })
        .collect();

    let title = if result.suggestions.len() > max_visible {
        format!(" {}/{} ", max_visible, result.suggestions.len())
    } else {
        format!(" {} ", result.suggestions.len())
    };

    f.render_widget(
        List::new(items).block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(BLUE))
                .title(Span::styled(title, Style::default().fg(FG3)))
                .style(Style::default().bg(BG_BAR)),
        ),
        popup,
    );
}

// ─── INSTANCES TAB ──────────────────────────────────────────────────

fn draw_instances(f: &mut Frame, area: Rect, app: &App) {
    use crate::instances::{InstanceFocus, SsmConnectionStatus};
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

    // Layout: profile bar (1) | left panel | divider | right panel
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

    // Left (regions + instances) | divider | Right (SSM terminal)
    let left_w = (rows[1].width * 36) / 100;
    let right_w = rows[1].width.saturating_sub(left_w + 1);
    let left_area = Rect::new(rows[1].x, rows[1].y, left_w, rows[1].height);
    let div_area = Rect::new(rows[1].x + left_w, rows[1].y, 1, rows[1].height);
    let right_area = Rect::new(rows[1].x + left_w + 1, rows[1].y, right_w, rows[1].height);

    // Divider
    let div_lines: Vec<Line> = (0..div_area.height)
        .map(|_| Line::from(Span::styled("│", Style::default().fg(FG4))))
        .collect();
    f.render_widget(Paragraph::new(div_lines), div_area);

    // Left panel: regions + instances
    let left_split = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(3)])
        .split(left_area);

    // Region selector (single line showing current, opens dropdown)
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
        for (i, inst) in is.instances.iter().enumerate() {
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

            let mut item = ListItem::new(Line::from(vec![
                sel_bar,
                Span::styled(format!(" {} ", inst.name), name_style),
                Span::styled(inst.instance_id.as_str(), Style::default().fg(FG3)),
                Span::styled(format!("  {}", inst.private_ip), Style::default().fg(FG4)),
            ]));
            if selected && inst_active {
                item = item.style(Style::default().bg(BG_HL));
            }
            items.push(item);
        }
    }
    f.render_widget(List::new(items), left_split[1]);

    // Right panel: SSM terminals
    // Layout: [session tabs (1, only when sessions exist)] [terminal (min)] [status bar (1)]
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

    // Session tab bar (only when sessions exist)
    let (term_area, status_area) = if has_sessions {
        let tab_area = right_split[0];
        let term_area = right_split[1];
        let status_area = right_split[2];

        let mut spans: Vec<Span> = Vec::new();
        for (i, session) in is.ssm_sessions.iter().enumerate() {
            let active = i == is.active_session_idx;
            // Truncate name to keep tab bar compact
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

    // Terminal area — render the active session's vt100 screen
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
                    if cell.bold() { s = s.add_modifier(Modifier::BOLD); }
                    if cell.italic() { s = s.add_modifier(Modifier::ITALIC); }
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

        // Cursor
        let (crow, ccol) = screen.cursor_position();
        let cx = term_area.x + ccol as u16;
        let cy = term_area.y + crow as u16;
        if cx < term_area.x + term_area.width && cy < term_area.y + term_area.height {
            f.set_cursor_position((cx, cy));
        }
    } else {
        // No sessions — show hint
        let hint = if let Some(ref err) = is.last_error {
            format!("  Error: {}", err)
        } else {
            "  Select an instance and press Enter to connect".to_string()
        };
        f.render_widget(
            Paragraph::new(Span::styled(hint, Style::default().fg(FG3))),
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
}

// ─── BODY (Sessions tab) ────────────────────────────────────────────

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
            SessionStatus::Connected => ("●", TEAL),
            SessionStatus::Expired  => ("○", AMBER),
            SessionStatus::Error(_) => ("×", RED),
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

        if let AliasKind::SsmSession { local_port: Some(p), .. } = &alias.kind {
            spans.push(Span::styled(format!(" :{}", p), Style::default().fg(TEAL)));
        }

        if matches!(status, SessionStatus::Running) {
            if let Some(u) = app.session_uptime(&alias.name) {
                spans.push(Span::styled(format!("  {}", u), Style::default().fg(FG3)));
            }
        }

        // SSO: show connected status
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

    // ── Name + status ──
    let (status_text, status_c) = match st {
        SessionStatus::Stopped   => ("stopped",      FG3),
        SessionStatus::Starting  => ("starting…",    AMBER),
        SessionStatus::Running   => ("running",      GREEN),
        SessionStatus::Connected => ("connected",    TEAL),
        SessionStatus::Expired   => ("expired",      AMBER),
        SessionStatus::Error(e)  => (e.as_str(),     RED),
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

    // SSO session info from STS check
    if let Some((info, _)) = app.token_expiry.get(&a.name) {
        lines.push(kv(ICON_GLOBE, "Identity", vec![
            Span::styled(info.as_str(), Style::default().fg(TEAL)),
        ]));
    }

    // Resolved temporary credentials (SSO connected sessions)
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
        let token_display = if c.session_token.len() > 28 {
            format!("{}…", &c.session_token[..28])
        } else {
            c.session_token.clone()
        };
        lines.push(kv(ICON_KEY, "Token", vec![
            Span::styled(token_display, Style::default().fg(FG3)),
        ]));
        lines.push(kv(ICON_CLOCK, "Expiration", vec![
            Span::styled(&c.expiration, Style::default().fg(FG2)),
        ]));
        lines.push(kv(ICON_CLOCK, "Local Time", vec![
            Span::styled(
                chrono::Local::now().format("%Y-%m-%dT%H:%M:%S").to_string(),
                Style::default().fg(FG2),
            ),
        ]));
    }

    // Show "verified" for connected, "expired" for expired
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
        Span::styled(format!("{:<14}", label), Style::default().fg(FG3)),
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
    let mut spans: Vec<Span> = vec![Span::styled(" ", Style::default())];

    match app.active_tab {
        AppTab::Sessions => {
            let selected_status = if !app.aliases.is_empty() {
                &app.session_statuses[app.selected_index]
            } else {
                &SessionStatus::Stopped
            };

            spans.push(key_span("↑↓"));
            spans.push(desc_span(" navigate  "));

            match selected_status {
                SessionStatus::Running | SessionStatus::Starting | SessionStatus::Connected => {
                    spans.push(key_span("s"));
                    spans.push(desc_span(" stop  "));
                }
                SessionStatus::Expired => {
                    spans.push(key_span("Enter"));
                    spans.push(desc_span(" re-login  "));
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

            // Show 'g' credentials hint only for connected SSO sessions
            let is_sso_connected = app.aliases.get(app.selected_index)
                .map(|a| matches!(a.kind, crate::parser::AliasKind::SsoLogin { .. }))
                .unwrap_or(false)
                && matches!(selected_status, SessionStatus::Connected);
            if is_sso_connected {
                spans.push(key_span("g"));
                spans.push(desc_span(" credentials  "));
            }

            spans.push(key_span("Tab"));
            spans.push(desc_span(" switch  "));
            spans.push(key_span("/"));
            spans.push(desc_span(" search  "));
        }
        AppTab::Instances => {
            if app.input_mode == InputMode::SsmInput {
                spans.push(key_span("Enter"));
                spans.push(desc_span(" send  "));
                spans.push(key_span("S+↑↓"));
                spans.push(desc_span(" scroll  "));
                spans.push(key_span("Ctrl+D"));
                spans.push(desc_span(" disconnect  "));
                spans.push(key_span("Esc"));
                spans.push(desc_span(" back  "));
            } else {
                use crate::instances::InstanceFocus;
                match app.instances_state.focus {
                    InstanceFocus::RegionList => {
                        spans.push(key_span("Enter"));
                        spans.push(desc_span(" open region  "));
                        spans.push(key_span("Tab"));
                        spans.push(desc_span(" focus  "));
                    }
                    InstanceFocus::InstanceList => {
                        spans.push(key_span("↑↓"));
                        spans.push(desc_span(" select  "));
                        spans.push(key_span("Enter"));
                        spans.push(desc_span(" connect  "));
                        spans.push(key_span("r"));
                        spans.push(desc_span(" refresh  "));
                        spans.push(key_span("Tab"));
                        spans.push(desc_span(" focus  "));
                    }
                    InstanceFocus::SsmTerminal => {
                        spans.push(key_span("i"));
                        spans.push(desc_span(" type  "));
                        spans.push(key_span("[/]"));
                        spans.push(desc_span(" sessions  "));
                        spans.push(key_span("Tab"));
                        spans.push(desc_span(" focus  "));
                    }
                }
            }
        }
        AppTab::Terminal => {
            if app.input_mode == InputMode::TerminalInput {
                spans.push(key_span("Enter"));
                spans.push(desc_span(" run  "));
                spans.push(key_span("Tab"));
                spans.push(desc_span(" accept  "));
                spans.push(key_span("S+↑↓"));
                spans.push(desc_span(" scroll  "));
                spans.push(key_span("Ctrl+H/L"));
                spans.push(desc_span(" switch  "));
                spans.push(key_span("Esc"));
                spans.push(desc_span(" normal  "));
            } else {
                spans.push(key_span("i"));
                spans.push(desc_span(" input  "));
                spans.push(key_span("j/k"));
                spans.push(desc_span(" scroll  "));
                spans.push(key_span("h/l"));
                spans.push(desc_span(" switch  "));
            }
        }
    }

    spans.push(key_span("F1/F2"));
    spans.push(desc_span(" tabs  "));
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

// ─── CREDENTIALS POPUP ──────────────────────────────────────────────

fn draw_credentials_popup(f: &mut Frame, area: Rect, app: &App) {
    let a = match app.aliases.get(app.selected_index) {
        Some(a) => a,
        None => return,
    };
    let creds = match app.session_credentials.get(&a.name) {
        Some(c) => c,
        None => return,
    };

    // Width: fill most of the screen for long tokens
    let w = area.width.saturating_sub(6).max(40);
    let inner_w = w.saturating_sub(4) as usize; // usable text width

    // Build content lines
    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(""));

    let label_style = Style::default().fg(FG2);

    // Helper: wrap a long value across multiple lines with indent
    let add_field = |lines: &mut Vec<Line>, label: &str, value: &str, val_color: Color| {
        let label_prefix = format!("  {:<16} ", label);
        let prefix_len = label_prefix.chars().count();
        let avail = inner_w.saturating_sub(prefix_len);

        // Split value into chunks that fit the available width
        let chars: Vec<char> = value.chars().collect();
        let first_chunk: String = chars.iter().take(avail).collect();
        let rest = &chars[first_chunk.chars().count()..];

        lines.push(Line::from(vec![
            Span::styled(label_prefix.clone(), label_style),
            Span::styled(first_chunk, Style::default().fg(val_color)),
        ]));

        // Continuation lines indented to align with value
        let indent = " ".repeat(prefix_len);
        for chunk in rest.chunks(inner_w.saturating_sub(prefix_len).max(1)) {
            let s: String = chunk.iter().collect();
            lines.push(Line::from(vec![
                Span::styled(indent.clone(), label_style),
                Span::styled(s, Style::default().fg(val_color)),
            ]));
        }
    };

    add_field(&mut lines, "AccessKeyId",    &creds.access_key_id,     BLUE);
    add_field(&mut lines, "SecretAccessKey", &creds.secret_access_key, AMBER);
    add_field(&mut lines, "SessionToken",   &creds.session_token,     FG);

    add_field(&mut lines, "Expiration", &creds.expiration, FG2);
    add_field(
        &mut lines,
        "Local Time",
        &chrono::Local::now().format("%Y-%m-%dT%H:%M:%S").to_string(),
        FG2,
    );

    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("  ", Style::default()),
        Span::styled("c", Style::default().fg(TEAL).add_modifier(Modifier::BOLD)),
        Span::styled(" copy as shell exports  ", Style::default().fg(FG3)),
        Span::styled("any other key", Style::default().fg(FG3)),
        Span::styled(" close", Style::default().fg(FG3)),
    ]));
    lines.push(Line::from(""));

    let content_h = lines.len() as u16 + 2; // +2 for border
    let h = content_h.min(area.height.saturating_sub(4));
    let x = (area.width.saturating_sub(w)) / 2;
    let y = (area.height.saturating_sub(h)) / 2;
    let r = Rect::new(x, y, w, h);

    dim_bg(f, area);
    f.render_widget(Clear, r);
    f.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .block(popup_block(&format!("  credentials — {}", a.name), TEAL)),
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
