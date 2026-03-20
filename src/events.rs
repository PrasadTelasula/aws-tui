use crate::app::{App, AppTab, ConfirmAction, InputMode};
use crate::containers::{ContainersFocus, ContainersSubTab};
use crate::instances;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Size;

pub fn key_to_pty_bytes(key: KeyEvent) -> Option<Vec<u8>> {
    match key.code {
        KeyCode::Char(c) if key.modifiers.contains(KeyModifiers::CONTROL) => {
            match c.to_ascii_lowercase() {
                'a'..='z' => Some(vec![c.to_ascii_lowercase() as u8 - b'a' + 1]),
                '[' => Some(vec![27]),
                '\\' => Some(vec![28]),
                ']' => Some(vec![29]),
                _ => None,
            }
        }
        KeyCode::Char(c) => {
            let mut buf = [0u8; 4];
            let s = c.encode_utf8(&mut buf);
            Some(s.as_bytes().to_vec())
        }
        KeyCode::Enter     => Some(vec![b'\r']),
        KeyCode::Backspace => Some(vec![127]),
        KeyCode::Delete    => Some(b"\x1b[3~".to_vec()),
        KeyCode::Esc       => Some(vec![27]),
        KeyCode::Tab       => Some(vec![b'\t']),
        KeyCode::BackTab   => Some(b"\x1b[Z".to_vec()),
        KeyCode::Up        => Some(b"\x1b[A".to_vec()),
        KeyCode::Down      => Some(b"\x1b[B".to_vec()),
        KeyCode::Right     => Some(b"\x1b[C".to_vec()),
        KeyCode::Left      => Some(b"\x1b[D".to_vec()),
        KeyCode::Home      => Some(b"\x1b[H".to_vec()),
        KeyCode::End       => Some(b"\x1b[F".to_vec()),
        KeyCode::PageUp    => Some(b"\x1b[5~".to_vec()),
        KeyCode::PageDown  => Some(b"\x1b[6~".to_vec()),
        _ => None,
    }
}

/// Handle a key event. Returns `Some(command)` if the TUI should suspend
/// to run an interactive SSM shell command, `None` otherwise.
pub async fn handle_key_event(
    app: &mut App,
    key: KeyEvent,
    terminal_size: Size,
) -> Option<String> {
    // ── Credentials popup ──────────────────────────────────────────
    if app.show_credentials_popup {
        if key.code == KeyCode::Char('c') {
            app.copy_credentials_to_clipboard();
        }
        app.show_credentials_popup = false;
        return None;
    }

    // ── Confirmation popup ──────────────────────────────────────────
    if app.show_confirm {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                let action = app.confirm_action.clone();
                app.show_confirm = false;
                app.confirm_action = ConfirmAction::None;
                match action {
                    ConfirmAction::StopAll => app.stop_all_sessions().await,
                    ConfirmAction::Quit    => app.should_quit = true,
                    ConfirmAction::None    => {}
                }
            }
            _ => {
                app.show_confirm = false;
                app.confirm_action = ConfirmAction::None;
            }
        }
        return None;
    }

    // ── Search mode (Sessions tab) ──────────────────────────────────
    if app.input_mode == InputMode::Search {
        match key.code {
            KeyCode::Esc => {
                app.input_mode = InputMode::Normal;
                app.search_query.clear();
                app.filtered_indices.clear();
            }
            KeyCode::Enter => {
                app.input_mode = InputMode::Normal;
            }
            KeyCode::Backspace => {
                app.search_query.pop();
                app.update_search();
            }
            KeyCode::Char(c) => {
                app.search_query.push(c);
                app.update_search();
            }
            _ => {}
        }
        return None;
    }

    // ── Global: F1/F2/F3 tab switching ─────────────────────────────
    match key.code {
        KeyCode::F(1) => {
            app.active_tab = AppTab::Sessions;
            app.input_mode = InputMode::Normal;
            return None;
        }
        KeyCode::F(2) => {
            app.active_tab = AppTab::Terminal;
            app.input_mode = InputMode::TerminalInput;
            return None;
        }
        KeyCode::F(3) => {
            app.active_tab = AppTab::Instances;
            app.input_mode = InputMode::Normal;
            if !app.instances_state.profiles.is_empty()
                && app.instances_state.instances.is_empty()
                && !app.instances_state.loading_instances
            {
                app.instances_state.fetch_instances();
            }
            return None;
        }
        KeyCode::F(4) => {
            app.active_tab = AppTab::Containers;
            app.input_mode = InputMode::Normal;
            // Auto-fetch if profile available and nothing loaded yet
            let cs = &app.containers_state;
            let should_fetch = !cs.profiles.is_empty()
                && cs.ecs_tree.is_empty()
                && !cs.loading_ecs_clusters
                && cs.eks_clusters.is_empty()
                && !cs.loading_eks_clusters;
            if should_fetch {
                app.containers_state.fetch_clusters();
            }
            return None;
        }
        _ => {}
    }

    // ── Terminal input mode ─────────────────────────────────────────
    if app.input_mode == InputMode::TerminalInput {
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            match key.code {
                KeyCode::Char('h') => { app.terminal_state.prev_terminal(); return None; }
                KeyCode::Char('l') => { app.terminal_state.next_terminal(); return None; }
                _ => {}
            }
        }

        if key.code == KeyCode::Enter {
            app.terminal_state.execute().await;
            return None;
        }

        if key.code == KeyCode::Esc {
            app.terminal_state.active_mut().completer.dismiss();
            app.input_mode = InputMode::Normal;
            return None;
        }

        let term = app.terminal_state.active_mut();

        if term.completer.visible {
            match key.code {
                KeyCode::Down => { term.completer.next(); return None; }
                KeyCode::Up   => { term.completer.prev(); return None; }
                KeyCode::Tab  => {
                    if let Some(new_input) = term.completer.accept_selected(&term.input) {
                        term.input = new_input;
                        term.cursor_pos = term.input.len();
                        term.completer.notify_keystroke();
                    }
                    return None;
                }
                _ => { term.completer.dismiss(); }
            }
        }

        match key.code {
            KeyCode::Backspace => term.backspace(),
            KeyCode::Delete    => term.delete(),
            KeyCode::Left      => term.cursor_left(),
            KeyCode::Right     => term.cursor_right(),
            KeyCode::Up if key.modifiers.contains(KeyModifiers::SHIFT)   => term.scroll_up(3),
            KeyCode::Down if key.modifiers.contains(KeyModifiers::SHIFT) => term.scroll_down(3),
            KeyCode::Up        => term.history_up(),
            KeyCode::Down      => term.history_down(),
            KeyCode::PageUp    => term.scroll_up(10),
            KeyCode::PageDown  => term.scroll_down(10),
            KeyCode::Tab => {
                if let Some(new_input) = term.completer.accept_selected(&term.input) {
                    term.input = new_input;
                    term.cursor_pos = term.input.len();
                    term.completer.notify_keystroke();
                }
            }
            KeyCode::Char('a') if key.modifiers.contains(KeyModifiers::CONTROL) => term.cursor_home(),
            KeyCode::Char('e') if key.modifiers.contains(KeyModifiers::CONTROL) => term.cursor_end(),
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => term.clear_line(),
            KeyCode::Char('w') if key.modifiers.contains(KeyModifiers::CONTROL) => term.delete_word_backward(),
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                term.input.clear();
                term.cursor_pos = 0;
                term.completer.dismiss();
            }
            KeyCode::Char(c) => term.insert_char(c),
            _ => {}
        }
        return None;
    }

    // ── Instance info popup ─────────────────────────────────────────
    if app.instances_state.show_info_popup
        && app.active_tab == AppTab::Instances
        && app.input_mode == InputMode::Normal
    {
        if let Some(ref mut p) = app.instances_state.info_popup {
            if p.search_active {
                match key.code {
                    KeyCode::Esc => {
                        p.search_active = false;
                        p.search_query.clear();
                        p.update_search();
                    }
                    KeyCode::Enter => { p.search_active = false; }
                    KeyCode::Backspace => {
                        p.search_query.pop();
                        p.update_search();
                    }
                    KeyCode::Char(c) => {
                        p.search_query.push(c);
                        p.update_search();
                    }
                    _ => {}
                }
                return None;
            }
        }

        match key.code {
            KeyCode::Esc => {
                if let Some(ref mut p) = app.instances_state.info_popup {
                    if !p.search_query.is_empty() {
                        p.search_query.clear();
                        p.update_search();
                    } else {
                        app.instances_state.show_info_popup = false;
                    }
                } else {
                    app.instances_state.show_info_popup = false;
                }
            }
            KeyCode::Char('q') => { app.instances_state.show_info_popup = false; }
            KeyCode::Char('/') => {
                if let Some(ref mut p) = app.instances_state.info_popup {
                    p.search_active = true;
                    p.search_query.clear();
                    p.update_search();
                }
            }
            KeyCode::Char('n') => {
                if let Some(ref mut p) = app.instances_state.info_popup { p.next_match(); }
            }
            KeyCode::Char('N') => {
                if let Some(ref mut p) = app.instances_state.info_popup { p.prev_match(); }
            }
            KeyCode::Tab => {
                if let Some(ref mut p) = app.instances_state.info_popup {
                    p.tab = match p.tab {
                        instances::InfoTab::Human => instances::InfoTab::Json,
                        instances::InfoTab::Json  => instances::InfoTab::Human,
                    };
                    p.scroll = 0;
                    p.update_search();
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if let Some(ref mut p) = app.instances_state.info_popup { p.scroll_down(1); }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if let Some(ref mut p) = app.instances_state.info_popup { p.scroll_up(1); }
            }
            KeyCode::PageDown => {
                if let Some(ref mut p) = app.instances_state.info_popup { p.scroll_down(10); }
            }
            KeyCode::PageUp => {
                if let Some(ref mut p) = app.instances_state.info_popup { p.scroll_up(10); }
            }
            _ => {}
        }
        return None;
    }

    // ── ECS info popup ──────────────────────────────────────────────
    if app.containers_state.show_info_popup
        && app.active_tab == AppTab::Containers
        && app.input_mode == InputMode::Normal
    {
        if let Some(ref mut p) = app.containers_state.info_popup {
            if p.search_active {
                match key.code {
                    KeyCode::Esc => {
                        p.search_active = false;
                        p.search_query.clear();
                        p.update_search();
                    }
                    KeyCode::Enter => { p.search_active = false; }
                    KeyCode::Backspace => {
                        p.search_query.pop();
                        p.update_search();
                    }
                    KeyCode::Char(c) => {
                        p.search_query.push(c);
                        p.update_search();
                    }
                    _ => {}
                }
                return None;
            }
        }

        match key.code {
            KeyCode::Esc => {
                if let Some(ref mut p) = app.containers_state.info_popup {
                    if !p.search_query.is_empty() {
                        p.search_query.clear();
                        p.update_search();
                    } else {
                        app.containers_state.show_info_popup = false;
                    }
                } else {
                    app.containers_state.show_info_popup = false;
                }
            }
            KeyCode::Char('q') => { app.containers_state.show_info_popup = false; }
            KeyCode::Char('/') => {
                if let Some(ref mut p) = app.containers_state.info_popup {
                    p.search_active = true;
                    p.search_query.clear();
                    p.update_search();
                }
            }
            KeyCode::Char('n') => {
                if let Some(ref mut p) = app.containers_state.info_popup { p.next_match(); }
            }
            KeyCode::Char('N') => {
                if let Some(ref mut p) = app.containers_state.info_popup { p.prev_match(); }
            }
            KeyCode::Tab => {
                if let Some(ref mut p) = app.containers_state.info_popup {
                    p.tab = match p.tab {
                        instances::InfoTab::Human => instances::InfoTab::Json,
                        instances::InfoTab::Json  => instances::InfoTab::Human,
                    };
                    p.scroll = 0;
                    p.update_search();
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if let Some(ref mut p) = app.containers_state.info_popup { p.scroll_down(1); }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if let Some(ref mut p) = app.containers_state.info_popup { p.scroll_up(1); }
            }
            KeyCode::PageDown => {
                if let Some(ref mut p) = app.containers_state.info_popup { p.scroll_down(10); }
            }
            KeyCode::PageUp => {
                if let Some(ref mut p) = app.containers_state.info_popup { p.scroll_up(10); }
            }
            _ => {}
        }
        return None;
    }

    // ── ECS tree search mode ─────────────────────────────────────────
    if app.containers_state.ecs_search_active
        && app.active_tab == AppTab::Containers
        && app.containers_state.sub_tab == ContainersSubTab::Ecs
        && app.input_mode == InputMode::Normal
    {
        match key.code {
            KeyCode::Esc | KeyCode::Enter => {
                app.containers_state.ecs_search_active = false;
                if key.code == KeyCode::Esc {
                    app.containers_state.ecs_search_query.clear();
                    app.containers_state.update_ecs_search();
                }
            }
            KeyCode::Backspace => {
                app.containers_state.ecs_search_query.pop();
                app.containers_state.update_ecs_search();
            }
            KeyCode::Char(c) => {
                app.containers_state.ecs_search_query.push(c);
                app.containers_state.update_ecs_search();
            }
            _ => {}
        }
        return None;
    }

    // ── Instance search mode ────────────────────────────────────────
    if app.instances_state.search_active
        && app.active_tab == AppTab::Instances
        && app.input_mode == InputMode::Normal
    {
        match key.code {
            KeyCode::Esc | KeyCode::Enter => {
                app.instances_state.search_active = false;
                if key.code == KeyCode::Esc {
                    app.instances_state.search_query.clear();
                    app.instances_state.filtered_instances.clear();
                }
            }
            KeyCode::Backspace => {
                app.instances_state.search_query.pop();
                app.instances_state.update_instance_search();
            }
            KeyCode::Char(c) => {
                app.instances_state.search_query.push(c);
                app.instances_state.update_instance_search();
            }
            _ => {}
        }
        return None;
    }

    // ── SSM Input mode ──────────────────────────────────────────────
    if app.input_mode == InputMode::SsmInput {
        match key.code {
            KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                app.instances_state.disconnect_ssm();
                if app.instances_state.ssm_sessions.is_empty() {
                    app.input_mode = InputMode::Normal;
                    app.instances_state.focus = instances::InstanceFocus::InstanceList;
                }
            }
            KeyCode::F(4) => { app.instances_state.prev_session(); }
            KeyCode::F(5) => { app.instances_state.next_session(); }
            KeyCode::Tab => {
                app.input_mode = InputMode::Normal;
                app.instances_state.cycle_focus();
            }
            _ => {
                if let Some(bytes) = key_to_pty_bytes(key) {
                    app.instances_state.write_input(&bytes);
                }
            }
        }
        return None;
    }

    // ── ECS Exec Input mode ─────────────────────────────────────────
    if app.input_mode == InputMode::EcsExecInput {
        match key.code {
            KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                app.containers_state.disconnect_ecs_exec();
                if app.containers_state.ecs_exec_sessions.is_empty() {
                    app.input_mode = InputMode::Normal;
                    app.containers_state.focus = ContainersFocus::ClusterList;
                }
            }
            KeyCode::F(4) => { app.containers_state.prev_exec_session(); }
            KeyCode::F(5) => { app.containers_state.next_exec_session(); }
            KeyCode::Tab => {
                app.input_mode = InputMode::Normal;
                app.containers_state.cycle_focus();
            }
            _ => {
                if let Some(bytes) = key_to_pty_bytes(key) {
                    app.containers_state.write_exec_input(&bytes);
                }
            }
        }
        return None;
    }

    // ── Normal mode ─────────────────────────────────────────────────
    match key.code {
        KeyCode::Char('q') => {
            if app.running_count > 0 {
                app.show_confirm = true;
                app.confirm_message = format!("Stop {} running session(s) and quit?", app.running_count);
                app.confirm_action = ConfirmAction::Quit;
            } else {
                app.should_quit = true;
            }
        }
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.should_quit = true;
        }
        KeyCode::Char('r')
            if key.modifiers.contains(KeyModifiers::CONTROL)
                && app.active_tab == AppTab::Sessions
                && app.input_mode == InputMode::Normal =>
        {
            app.reload_aliases();
        }

        // ── Terminal tab ──
        KeyCode::Char('i') if app.active_tab == AppTab::Terminal => {
            app.input_mode = InputMode::TerminalInput;
        }
        KeyCode::Up | KeyCode::Char('k') if app.active_tab == AppTab::Terminal => {
            app.terminal_state.active_mut().scroll_up(3);
        }
        KeyCode::Down | KeyCode::Char('j') if app.active_tab == AppTab::Terminal => {
            app.terminal_state.active_mut().scroll_down(3);
        }
        KeyCode::Char('h') if app.active_tab == AppTab::Terminal => {
            app.terminal_state.prev_terminal();
        }
        KeyCode::Char('l') if app.active_tab == AppTab::Terminal => {
            app.terminal_state.next_terminal();
        }

        // ── Instances tab ──
        KeyCode::Tab if app.active_tab == AppTab::Instances => {
            app.instances_state.cycle_focus();
            if app.instances_state.focus == instances::InstanceFocus::SsmTerminal {
                app.input_mode = InputMode::SsmInput;
            }
        }
        KeyCode::Up | KeyCode::Char('k') if app.active_tab == AppTab::Instances => {
            if app.instances_state.region_dropdown_open {
                app.instances_state.prev_region();
            } else {
                match app.instances_state.focus {
                    instances::InstanceFocus::RegionList   => {}
                    instances::InstanceFocus::InstanceList => app.instances_state.prev_instance(),
                    instances::InstanceFocus::SsmTerminal  => {}
                }
            }
        }
        KeyCode::Down | KeyCode::Char('j') if app.active_tab == AppTab::Instances => {
            if app.instances_state.region_dropdown_open {
                app.instances_state.next_region();
            } else {
                match app.instances_state.focus {
                    instances::InstanceFocus::RegionList   => {}
                    instances::InstanceFocus::InstanceList => app.instances_state.next_instance(),
                    instances::InstanceFocus::SsmTerminal  => {}
                }
            }
        }
        KeyCode::Enter if app.active_tab == AppTab::Instances => {
            if app.instances_state.region_dropdown_open {
                app.instances_state.region_dropdown_open = false;
                app.instances_state.fetch_instances();
            } else {
                match app.instances_state.focus {
                    instances::InstanceFocus::RegionList => {
                        app.instances_state.region_dropdown_open = true;
                    }
                    instances::InstanceFocus::InstanceList => {
                        let rows = terminal_size.height.saturating_sub(5);
                        let cols = (terminal_size.width * 64 / 100).saturating_sub(2);
                        app.instances_state.connect_ssm(rows.max(10), cols.max(20));
                        app.instances_state.focus = instances::InstanceFocus::SsmTerminal;
                        app.input_mode = InputMode::SsmInput;
                    }
                    instances::InstanceFocus::SsmTerminal => {}
                }
            }
        }
        KeyCode::Esc if app.active_tab == AppTab::Instances => {
            if app.instances_state.region_dropdown_open {
                app.instances_state.region_dropdown_open = false;
            }
        }
        KeyCode::Char('/')
            if app.active_tab == AppTab::Instances
                && app.instances_state.focus == instances::InstanceFocus::InstanceList =>
        {
            app.instances_state.search_active = true;
            app.instances_state.search_query.clear();
            app.instances_state.filtered_instances.clear();
        }
        KeyCode::Esc
            if app.active_tab == AppTab::Instances
                && !app.instances_state.search_query.is_empty() =>
        {
            app.instances_state.search_query.clear();
            app.instances_state.filtered_instances.clear();
        }
        KeyCode::Char('r') if app.active_tab == AppTab::Instances => {
            app.instances_state.fetch_instances();
        }
        KeyCode::Char('i') if app.active_tab == AppTab::Instances => {
            match app.instances_state.focus {
                instances::InstanceFocus::InstanceList => {
                    if !app.instances_state.instances.is_empty() {
                        app.instances_state.fetch_instance_info();
                    }
                }
                _ => {
                    if !app.instances_state.ssm_sessions.is_empty() {
                        app.instances_state.focus = instances::InstanceFocus::SsmTerminal;
                        app.input_mode = InputMode::SsmInput;
                    }
                }
            }
        }
        KeyCode::Char('[') if app.active_tab == AppTab::Instances => {
            app.instances_state.prev_session();
        }
        KeyCode::Char(']') if app.active_tab == AppTab::Instances => {
            app.instances_state.next_session();
        }
        KeyCode::Char('h')
            if key.modifiers.contains(KeyModifiers::CONTROL)
                && app.active_tab == AppTab::Instances =>
        {
            app.instances_state.prev_profile();
            app.instances_state.fetch_instances();
        }
        KeyCode::Char('l')
            if key.modifiers.contains(KeyModifiers::CONTROL)
                && app.active_tab == AppTab::Instances =>
        {
            app.instances_state.next_profile();
            app.instances_state.fetch_instances();
        }

        // ── Containers tab ──
        KeyCode::Char('1') if app.active_tab == AppTab::Containers => {
            app.containers_state.sub_tab = ContainersSubTab::Ecs;
            app.containers_state.focus = ContainersFocus::ClusterList;
            if app.containers_state.ecs_tree.is_empty()
                && !app.containers_state.loading_ecs_clusters
                && !app.containers_state.profiles.is_empty()
            {
                app.containers_state.fetch_ecs_clusters();
            }
        }
        KeyCode::Char('2') if app.active_tab == AppTab::Containers => {
            app.containers_state.sub_tab = ContainersSubTab::Eks;
            app.containers_state.focus = ContainersFocus::ClusterList;
            if app.containers_state.eks_clusters.is_empty()
                && !app.containers_state.loading_eks_clusters
                && !app.containers_state.profiles.is_empty()
            {
                app.containers_state.fetch_eks_clusters();
            }
        }
        KeyCode::Tab if app.active_tab == AppTab::Containers => {
            app.containers_state.cycle_focus();
            if app.containers_state.focus == ContainersFocus::EcsTerminal {
                app.input_mode = InputMode::EcsExecInput;
            }
        }
        KeyCode::Up | KeyCode::Char('k') if app.active_tab == AppTab::Containers => {
            if app.containers_state.region_dropdown_open {
                app.containers_state.prev_region();
            } else {
                match app.containers_state.focus {
                    ContainersFocus::RegionList  => {}
                    ContainersFocus::SubTabBar   => {}
                    ContainersFocus::ClusterList => app.containers_state.prev_cluster(),
                    ContainersFocus::DetailList  => app.containers_state.prev_detail(),
                    ContainersFocus::EcsTerminal => {}
                }
            }
        }
        KeyCode::Down | KeyCode::Char('j') if app.active_tab == AppTab::Containers => {
            if app.containers_state.region_dropdown_open {
                app.containers_state.next_region();
            } else {
                match app.containers_state.focus {
                    ContainersFocus::RegionList  => {}
                    ContainersFocus::SubTabBar   => {}
                    ContainersFocus::ClusterList => app.containers_state.next_cluster(),
                    ContainersFocus::DetailList  => app.containers_state.next_detail(),
                    ContainersFocus::EcsTerminal => {}
                }
            }
        }
        // Left collapses ECS tree item when tree is focused
        KeyCode::Left
            if app.active_tab == AppTab::Containers
                && app.containers_state.focus == ContainersFocus::ClusterList
                && app.containers_state.sub_tab == ContainersSubTab::Ecs =>
        {
            app.containers_state.ecs_collapse_selected();
        }
        // Left/Right switch ECS<->EKS when focused on sub-tab bar
        KeyCode::Left | KeyCode::Right
            if app.active_tab == AppTab::Containers
                && app.containers_state.focus == ContainersFocus::SubTabBar =>
        {
            app.containers_state.switch_sub_tab();
            if !app.containers_state.profiles.is_empty() {
                app.containers_state.fetch_clusters();
            }
        }
        KeyCode::Enter if app.active_tab == AppTab::Containers => {
            if app.containers_state.region_dropdown_open {
                app.containers_state.region_dropdown_open = false;
                app.containers_state.fetch_clusters();
            } else {
                match app.containers_state.focus {
                    ContainersFocus::RegionList  => {
                        app.containers_state.region_dropdown_open = true;
                    }
                    ContainersFocus::SubTabBar   => {
                        app.containers_state.switch_sub_tab();
                        if !app.containers_state.profiles.is_empty() {
                            app.containers_state.fetch_clusters();
                        }
                    }
                    ContainersFocus::ClusterList => {
                        match app.containers_state.sub_tab {
                            ContainersSubTab::Ecs => {
                                app.containers_state.ecs_toggle_selected();
                            }
                            ContainersSubTab::Eks => {
                                app.containers_state.fetch_detail_for_selected();
                                app.containers_state.focus = ContainersFocus::DetailList;
                            }
                        }
                    }
                    ContainersFocus::DetailList  => {}
                    ContainersFocus::EcsTerminal => {
                        app.input_mode = InputMode::EcsExecInput;
                    }
                }
            }
        }
        KeyCode::Esc if app.active_tab == AppTab::Containers => {
            if app.containers_state.region_dropdown_open {
                app.containers_state.region_dropdown_open = false;
            } else if app.containers_state.focus == ContainersFocus::DetailList {
                app.containers_state.focus = ContainersFocus::ClusterList;
            } else if app.containers_state.focus == ContainersFocus::ClusterList
                && app.containers_state.sub_tab == ContainersSubTab::Ecs
            {
                app.containers_state.ecs_collapse_selected();
            }
        }
        KeyCode::Char('/')
            if app.active_tab == AppTab::Containers
                && app.containers_state.sub_tab == ContainersSubTab::Ecs
                && app.containers_state.focus == ContainersFocus::ClusterList =>
        {
            app.containers_state.ecs_search_active = true;
            app.containers_state.ecs_search_query.clear();
            app.containers_state.update_ecs_search();
        }
        KeyCode::Esc
            if app.active_tab == AppTab::Containers
                && app.containers_state.sub_tab == ContainersSubTab::Ecs
                && !app.containers_state.ecs_search_query.is_empty() =>
        {
            app.containers_state.ecs_search_query.clear();
            app.containers_state.update_ecs_search();
        }
        KeyCode::Char('i')
            if app.active_tab == AppTab::Containers
                && app.containers_state.sub_tab == ContainersSubTab::Ecs
                && app.containers_state.focus == ContainersFocus::ClusterList =>
        {
            if !app.containers_state.ecs_tree.is_empty() {
                app.containers_state.fetch_ecs_info();
            }
        }
        KeyCode::Char('e')
            if app.active_tab == AppTab::Containers
                && app.containers_state.sub_tab == ContainersSubTab::Ecs
                && app.containers_state.focus == ContainersFocus::ClusterList =>
        {
            let rows = terminal_size.height.saturating_sub(9);
            let inner_w  = terminal_size.width.saturating_sub(2);
            let left_w   = (inner_w * 36) / 100;
            let cols = inner_w.saturating_sub(left_w + 1);
            app.containers_state.connect_ecs_exec(rows.max(10), cols.max(20));
            if app.containers_state.focus == ContainersFocus::EcsTerminal {
                app.input_mode = InputMode::EcsExecInput;
            }
        }
        KeyCode::Char('e')
            if app.active_tab == AppTab::Containers
                && app.containers_state.sub_tab == ContainersSubTab::Ecs
                && app.containers_state.focus == ContainersFocus::EcsTerminal =>
        {
            // 'e' while terminal focused: re-enter EcsExecInput mode
            app.input_mode = InputMode::EcsExecInput;
        }
        KeyCode::Char('r') if app.active_tab == AppTab::Containers => {
            app.containers_state.fetch_clusters();
        }
        KeyCode::Char('h')
            if key.modifiers.contains(KeyModifiers::CONTROL)
                && app.active_tab == AppTab::Containers =>
        {
            app.containers_state.prev_profile();
            app.containers_state.fetch_clusters();
        }
        KeyCode::Char('l')
            if key.modifiers.contains(KeyModifiers::CONTROL)
                && app.active_tab == AppTab::Containers =>
        {
            app.containers_state.next_profile();
            app.containers_state.fetch_clusters();
        }

        // ── Sessions tab ──
        KeyCode::Down | KeyCode::Char('j') if app.active_tab == AppTab::Sessions => {
            app.next();
        }
        KeyCode::Up | KeyCode::Char('k') if app.active_tab == AppTab::Sessions => {
            app.previous();
        }
        KeyCode::Tab | KeyCode::BackTab if app.active_tab == AppTab::Sessions => {
            app.toggle_panel();
        }
        KeyCode::Enter if app.active_tab == AppTab::Sessions => {
            app.start_selected().await;
            if let Some(cmd) = app.pending_ssm_command.take() {
                return Some(cmd);
            }
        }
        KeyCode::Char('s') if app.active_tab == AppTab::Sessions => {
            app.stop_selected().await;
        }
        KeyCode::Char('S') if app.active_tab == AppTab::Sessions => {
            if app.running_count > 0 {
                app.show_confirm = true;
                app.confirm_message = format!("Stop all {} running session(s)?", app.running_count);
                app.confirm_action = ConfirmAction::StopAll;
            }
        }
        KeyCode::Char('/') if app.active_tab == AppTab::Sessions => {
            app.input_mode = InputMode::Search;
            app.search_query.clear();
        }
        KeyCode::Esc => {
            if !app.search_query.is_empty() {
                app.search_query.clear();
                app.filtered_indices.clear();
            }
        }
        KeyCode::Char('g') if app.active_tab == AppTab::Sessions => {
            let is_cred_connected = app.aliases.get(app.selected_index)
                .map(|a| matches!(
                    a.kind,
                    crate::parser::AliasKind::SsoLogin { .. } | crate::parser::AliasKind::IamProfile { .. }
                ))
                .unwrap_or(false)
                && app.session_credentials.contains_key(
                    app.aliases.get(app.selected_index).map(|a| a.name.as_str()).unwrap_or(""),
                );
            if is_cred_connected {
                app.show_credentials_popup = true;
            } else {
                app.selected_index = 0;
            }
        }
        KeyCode::Char('G') if app.active_tab == AppTab::Sessions => {
            if !app.aliases.is_empty() {
                app.selected_index = app.aliases.len() - 1;
            }
        }
        _ => {}
    }

    None
}
