use super::*;
use crate::app::{App, InputMode};

pub(super) fn draw_terminal(f: &mut Frame, area: Rect, app: &App) {
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
                Constraint::Length(1),
                Constraint::Min(3),
                Constraint::Length(1),
                Constraint::Length(1),
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
