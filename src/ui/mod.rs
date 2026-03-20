mod containers;
mod instances;
mod sessions;
mod terminal;

use crate::app::*;
use crate::parser::AliasKind;
use crate::session::SessionStatus;

pub(crate) use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, BorderType, Borders, Clear, List, ListItem, Paragraph, Scrollbar,
        ScrollbarOrientation, ScrollbarState, Wrap,
    },
    Frame,
};

pub(crate) fn vt100_color(c: vt100::Color) -> Color {
    match c {
        vt100::Color::Default => Color::Reset,
        vt100::Color::Idx(n) => Color::Indexed(n),
        vt100::Color::Rgb(r, g, b) => Color::Rgb(r, g, b),
    }
}

// ─── Catppuccin Macchiato palette ───────────────────────────────────
pub(crate) const BG: Color = Color::Rgb(36, 39, 58);
pub(crate) const BG_ALT: Color = Color::Rgb(40, 44, 64);
pub(crate) const BG_HL: Color = Color::Rgb(49, 50, 68);
pub(crate) const BG_BAR: Color = Color::Rgb(30, 32, 48);
pub(crate) const FG: Color = Color::Rgb(202, 211, 245);
pub(crate) const FG2: Color = Color::Rgb(147, 154, 183);
pub(crate) const FG3: Color = Color::Rgb(91, 96, 120);
pub(crate) const FG4: Color = Color::Rgb(54, 58, 79);
pub(crate) const BLUE: Color = Color::Rgb(138, 173, 244);
pub(crate) const GREEN: Color = Color::Rgb(166, 218, 149);
pub(crate) const RED: Color = Color::Rgb(237, 135, 150);
pub(crate) const AMBER: Color = Color::Rgb(238, 212, 159);
pub(crate) const TEAL: Color = Color::Rgb(139, 213, 202);
pub(crate) const MAUVE: Color = Color::Rgb(198, 160, 246);

// ─── Nerd Font icons ────────────────────────────────────────────────
pub(crate) const ICON_CLOUD: &str = "\u{f0c2}";
pub(crate) const ICON_KEY: &str = "\u{f084}";
pub(crate) const ICON_PLUG: &str = "\u{f1e6}";
pub(crate) const ICON_SERVER: &str = "\u{f233}";
pub(crate) const ICON_COG: &str = "\u{f013}";
pub(crate) const ICON_CLOCK: &str = "\u{f017}";
pub(crate) const ICON_SEARCH: &str = "\u{f002}";
#[allow(dead_code)]
pub(crate) const ICON_FOLDER: &str = "\u{f07b}";
pub(crate) const ICON_FOLDER_OPEN: &str = "\u{f07c}";
pub(crate) const ICON_TERM: &str = "\u{f120}";
pub(crate) const ICON_GLOBE: &str = "\u{f0ac}";
pub(crate) const ICON_HASH: &str = "\u{f292}";
pub(crate) const ICON_DATABASE: &str = "\u{f1c0}";
pub(crate) const ICON_SHIELD: &str = "\u{f132}";
pub(crate) const ICON_DESKTOP: &str = "\u{f108}";
pub(crate) const ICON_NETWORK: &str = "\u{f0e8}";
pub(crate) const ICON_TAG: &str = "\u{f02b}";
pub(crate) const ICON_LINUX: &str = "\u{f17c}";
pub(crate) const ICON_WINDOWS: &str = "\u{f17a}";

// ════════════════════════════════════════════════════════════════════

pub fn draw(f: &mut Frame, app: &App) {
    let size = f.area();
    f.render_widget(Block::default().style(Style::default().bg(BG)), size);

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(8),
            Constraint::Length(1),
        ])
        .split(size);

    draw_header(f, layout[0], app);
    draw_tab_bar(f, layout[1], app);
    draw_separator(f, layout[2]);

    match app.active_tab {
        AppTab::Sessions   => sessions::draw_body(f, layout[3], app),
        AppTab::Terminal   => terminal::draw_terminal(f, layout[3], app),
        AppTab::Instances  => instances::draw_instances(f, layout[3], app),
        AppTab::Containers => containers::draw_containers(f, layout[3], app),
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
        Span::styled("  ", Style::default()),
        Span::styled(bar(AppTab::Containers), Style::default().fg(BLUE)),
        Span::styled(format!(" \u{f1b2} Containers "), tab_style(AppTab::Containers)),
        Span::styled("    ", Style::default()),
        Span::styled("F1-F4", Style::default().fg(FG4)),
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

            let is_sso_connected = app.aliases.get(app.selected_index)
                .map(|a| matches!(a.kind, AliasKind::SsoLogin { .. } | AliasKind::IamProfile { .. }))
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
            spans.push(key_span("Ctrl+R"));
            spans.push(desc_span(" reload  "));
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
        AppTab::Containers => {
            use crate::containers::ContainersFocus;
            match app.containers_state.focus {
                ContainersFocus::ClusterList => {
                    spans.push(key_span("↑↓"));
                    spans.push(desc_span(" cluster  "));
                    spans.push(key_span("Enter"));
                    spans.push(desc_span(" load detail  "));
                    spans.push(key_span("Tab"));
                    spans.push(desc_span(" focus  "));
                    spans.push(key_span("r"));
                    spans.push(desc_span(" refresh  "));
                    spans.push(key_span("1/2"));
                    spans.push(desc_span(" ECS/EKS  "));
                }
                ContainersFocus::DetailList => {
                    spans.push(key_span("↑↓"));
                    spans.push(desc_span(" select  "));
                    spans.push(key_span("Tab"));
                    spans.push(desc_span(" focus  "));
                    spans.push(key_span("r"));
                    spans.push(desc_span(" refresh  "));
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

    spans.push(key_span("F1-F4"));
    spans.push(desc_span(" tabs  "));
    spans.push(key_span("q"));
    spans.push(desc_span(" quit"));

    f.render_widget(
        Paragraph::new(Line::from(spans)).style(Style::default().bg(BG_BAR)),
        area,
    );
}

fn key_span(k: &str) -> Span<'_> {
    Span::styled(
        format!("[{}]", k),
        Style::default().fg(AMBER).add_modifier(Modifier::BOLD),
    )
}

fn desc_span(d: &str) -> Span<'_> {
    Span::styled(d, Style::default().fg(FG2))
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

    let w = area.width.saturating_sub(6).max(40);
    let inner_w = w.saturating_sub(4) as usize;

    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(""));

    let label_style = Style::default().fg(FG2);

    let add_field = |lines: &mut Vec<Line>, label: &str, value: &str, val_color: Color| {
        let label_prefix = format!("  {:<16} ", label);
        let prefix_len = label_prefix.chars().count();
        let avail = inner_w.saturating_sub(prefix_len);

        let chars: Vec<char> = value.chars().collect();
        let first_chunk: String = chars.iter().take(avail).collect();
        let rest = &chars[first_chunk.chars().count()..];

        lines.push(Line::from(vec![
            Span::styled(label_prefix.clone(), label_style),
            Span::styled(first_chunk, Style::default().fg(val_color)),
        ]));

        let indent = " ".repeat(prefix_len);
        for chunk in rest.chunks(inner_w.saturating_sub(prefix_len).max(1)) {
            let s: String = chunk.iter().collect();
            lines.push(Line::from(vec![
                Span::styled(indent.clone(), label_style),
                Span::styled(s, Style::default().fg(val_color)),
            ]));
        }
    };

    add_field(&mut lines, "AccessKeyId",     &creds.access_key_id,     BLUE);
    add_field(&mut lines, "SecretAccessKey", &creds.secret_access_key, AMBER);

    if !matches!(a.kind, AliasKind::IamProfile { .. }) {
        add_field(&mut lines, "SessionToken", &creds.session_token, FG);
        add_field(&mut lines, "Expiration",   &creds.expiration,    FG2);
    }
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

    let content_h = lines.len() as u16 + 2;
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
    let max_msg_w = area.width.saturating_sub(10) as usize;
    let msg = if toast.message.len() > max_msg_w && max_msg_w > 3 {
        format!("{}…", &toast.message[..max_msg_w.saturating_sub(1)])
    } else {
        toast.message.clone()
    };
    let w = (msg.len() as u16 + 6).min(area.width.saturating_sub(4));
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
            format!("  {}", msg),
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

// ─── Shared helpers ─────────────────────────────────────────────────

pub(crate) fn popup_block(title: &str, color: Color) -> Block<'_> {
    Block::default()
        .title(Span::styled(format!(" {} ", title), Style::default().fg(color)))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(color))
        .style(Style::default().bg(BG_BAR))
}

pub(crate) fn dim_bg(f: &mut Frame, area: Rect) {
    f.render_widget(
        Block::default().style(
            Style::default()
                .bg(Color::Rgb(20, 22, 34))
                .add_modifier(Modifier::DIM),
        ),
        area,
    );
}

pub(crate) fn thin_rule(f: &mut Frame, area: Rect) {
    let w = area.width.saturating_sub(2) as usize;
    f.render_widget(
        Paragraph::new(Span::styled(format!(" {}", "─".repeat(w)), Style::default().fg(FG4))),
        area,
    );
}
