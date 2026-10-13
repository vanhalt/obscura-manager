use crate::app::{App, ConfirmAction, Mode, Tab};
use crate::cli::TaskKind;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

fn spawn_from_form(app: &mut App) {
    let mut params = crate::app::params_from_form(app.form_kind, &app.form_fields);
    // default artifact outputs
    let default_out = crate::cli::default_output_path(&app.root, &params);
    if let crate::cli::TaskParams::Fetch(ref mut p) = params {
        if p.output.is_empty()
            && p.file.is_empty()
            && let Some(path) = default_out
        {
            p.output = path.display().to_string();
        } else if !p.output.is_empty() {
            // Relative --output names belong in artifacts/dumps/, not the CWD.
            p.output = crate::cli::resolve_dump_output(&app.root, &p.output)
                .display()
                .to_string();
        }
        if !p.screenshot.is_empty() && !std::path::Path::new(&p.screenshot).is_absolute() {
            let path = crate::config::artifacts_dir(&app.root)
                .join("screenshots")
                .join(&p.screenshot);
            p.screenshot = path.display().to_string();
        }
    }
    let Some(bin) = app.binary.clone() else {
        app.toast("obscura binary not found — set OBSCURA_BIN");
        return;
    };
    let tx = app.tx.clone();
    match app
        .manager
        .spawn(&app.root, &bin, params, &app.config.env_overrides, tx)
    {
        Ok(id) => {
            app.toast(format!("Spawned {}", &id.to_string()[..8]));
            app.manager.save_history(&app.root);
            app.mode = Mode::Normal;
            app.tab = Tab::Tasks;
            // select last
            app.selected = app.manager.instances.len().saturating_sub(1);
        }
        Err(e) => app.toast(format!("Spawn failed: {e}")),
    }
}

pub fn handle_key(app: &mut App, key: KeyEvent) {
    // Editing modes first
    match app.mode {
        Mode::EditingField => {
            match key.code {
                KeyCode::Enter | KeyCode::Esc => {
                    if key.code == KeyCode::Enter
                        && let Some(f) = app.form_fields.get_mut(app.form_focus)
                    {
                        f.value = app.edit_buffer.clone();
                    }
                    app.mode = Mode::CreateForm;
                }
                KeyCode::Backspace => {
                    app.edit_buffer.pop();
                }
                KeyCode::Char(c) => app.edit_buffer.push(c),
                _ => {}
            }
            return;
        }
        Mode::Filter => {
            match key.code {
                KeyCode::Enter | KeyCode::Esc => app.mode = Mode::Normal,
                KeyCode::Backspace => {
                    app.filter.pop();
                }
                KeyCode::Char(c) => app.filter.push(c),
                _ => {}
            }
            return;
        }
        Mode::Confirm => {
            match key.code {
                KeyCode::Char('y') | KeyCode::Enter => {
                    if let Some(act) = app.confirm_action.take() {
                        match act {
                            ConfirmAction::Kill(id) => {
                                let _ = app.manager.kill(id);
                                app.toast("Killed");
                            }
                            ConfirmAction::DeleteInstance(id) => {
                                app.manager.delete_history(id);
                                app.toast("Deleted");
                            }
                            ConfirmAction::DeleteArtifact(p) => {
                                let _ = std::fs::remove_file(&p);
                                app.refresh_artifacts();
                                app.toast("Artifact deleted");
                            }
                        }
                        app.manager.save_history(&app.root);
                    }
                    app.mode = Mode::Normal;
                }
                KeyCode::Char('n') | KeyCode::Esc => {
                    app.confirm_action = None;
                    app.mode = Mode::Normal;
                }
                _ => {}
            }
            return;
        }
        Mode::CreateForm => {
            handle_form_key(app, key);
            return;
        }
        Mode::Normal => {}
    }

    // Global keys
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        app.should_quit = true;
        return;
    }
    // CDP param editing overlay
    if app.cdp_editing_params {
        match key.code {
            KeyCode::Esc => app.cdp_editing_params = false,
            KeyCode::Enter => app.cdp_editing_params = false,
            KeyCode::Backspace => {
                app.cdp_params_json.pop();
            }
            KeyCode::Char(c) => app.cdp_params_json.push(c),
            _ => {}
        }
        return;
    }
    if app.config_editing {
        match key.code {
            KeyCode::Esc | KeyCode::Enter => {
                if key.code == KeyCode::Enter {
                    let keys: Vec<_> = crate::config::KNOWN_ENV_VARS.to_vec();
                    if let Some(k) = keys.get(app.config_selected) {
                        if app.edit_buffer.is_empty() {
                            app.config.env_overrides.remove(*k);
                        } else {
                            app.config
                                .env_overrides
                                .insert(k.to_string(), app.edit_buffer.clone());
                        }
                        let _ = app.config.save(&app.root);
                    }
                }
                app.config_editing = false;
            }
            KeyCode::Backspace => {
                app.edit_buffer.pop();
            }
            KeyCode::Char(c) => app.edit_buffer.push(c),
            _ => {}
        }
        return;
    }

    match key.code {
        KeyCode::Char('q') => app.should_quit = true,
        KeyCode::Char('?') => app.tab = Tab::Help,
        KeyCode::Char(c) if ('1'..='7').contains(&c) => {
            let idx = c as usize - '1' as usize;
            app.tab = Tab::from_index(idx);
        }
        KeyCode::Tab => {
            app.tab = Tab::from_index(app.tab.index() + 1);
        }
        KeyCode::BackTab => {
            app.tab = Tab::from_index(app.tab.index() + 6);
        }
        _ => match app.tab {
            Tab::Tasks | Tab::Dashboard | Tab::Logs => handle_tasks_key(app, key),
            Tab::Artifacts => handle_artifacts_key(app, key),
            Tab::Cdp => handle_cdp_key(app, key),
            Tab::Config => handle_config_key(app, key),
            Tab::Help => {}
        },
    }
}

fn handle_form_key(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => app.mode = Mode::Normal,
        KeyCode::Tab | KeyCode::Down => {
            if !app.form_fields.is_empty() {
                app.form_focus = (app.form_focus + 1) % app.form_fields.len();
            }
        }
        KeyCode::BackTab | KeyCode::Up => {
            if !app.form_fields.is_empty() {
                app.form_focus =
                    (app.form_focus + app.form_fields.len() - 1) % app.form_fields.len();
            }
        }
        KeyCode::Char('j')
            if !app
                .form_fields
                .get(app.form_focus)
                .is_some_and(|f| !f.is_bool && f.options.is_empty()) => {}
        KeyCode::Enter | KeyCode::Char(' ') => {
            if let Some(f) = app.form_fields.get_mut(app.form_focus) {
                if f.is_bool || !f.options.is_empty() {
                    f.cycle();
                } else {
                    app.edit_buffer = f.value.clone();
                    app.mode = Mode::EditingField;
                }
            }
        }
        KeyCode::Char('e') => {
            if let Some(f) = app.form_fields.get(app.form_focus)
                && !f.is_bool
                && f.options.is_empty()
            {
                app.edit_buffer = f.value.clone();
                app.mode = Mode::EditingField;
            }
        }
        KeyCode::Char('1') => {
            app.open_form(TaskKind::Fetch);
        }
        KeyCode::Char('2') => {
            app.open_form(TaskKind::Serve);
        }
        KeyCode::Char('3') => {
            app.open_form(TaskKind::Scrape);
        }
        KeyCode::Char('4') => {
            app.open_form(TaskKind::Mcp);
        }
        KeyCode::F(5) => spawn_from_form(app),
        KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => spawn_from_form(app),
        _ => {
            // Ctrl+S alternative handled above; also allow 'S' to spawn
            if key.code == KeyCode::Char('S') {
                spawn_from_form(app);
            }
        }
    }
}

fn handle_tasks_key(app: &mut App, key: KeyEvent) {
    let count = app.filtered_instances().len();
    match key.code {
        KeyCode::Down | KeyCode::Char('j') => {
            if count > 0 {
                app.selected = (app.selected + 1).min(count - 1);
            }
            app.log_scroll = 0;
        }
        KeyCode::Up | KeyCode::Char('k') => {
            app.selected = app.selected.saturating_sub(1);
            app.log_scroll = 0;
        }
        KeyCode::Char('n') => app.open_form(TaskKind::Fetch),
        KeyCode::Char('f') => app.open_form(TaskKind::Fetch),
        KeyCode::Char('v') => app.open_form(TaskKind::Serve), // serVe
        KeyCode::Char('p') => app.open_form(TaskKind::Scrape), // scraPe
        KeyCode::Char('m') => app.open_form(TaskKind::Mcp),
        KeyCode::Char('/') => app.mode = Mode::Filter,
        KeyCode::Char('s') => {
            if let Some(id) = app.selected_instance_id() {
                app.confirm_action = Some(ConfirmAction::Kill(id));
                app.mode = Mode::Confirm;
            }
        }
        KeyCode::Char('r') => {
            if let Some(id) = app.selected_instance_id()
                && let Some(params) = app.manager.restart_params(id)
                && let Some(bin) = app.binary.clone()
            {
                let tx = app.tx.clone();
                match app
                    .manager
                    .spawn(&app.root, &bin, params, &app.config.env_overrides, tx)
                {
                    Ok(new_id) => {
                        app.toast(format!(
                            "Spawned new {} (no reattach)",
                            &new_id.to_string()[..8]
                        ));
                        app.selected = app.manager.instances.len().saturating_sub(1);
                        app.manager.save_history(&app.root);
                    }
                    Err(e) => app.toast(format!("Restart failed: {e}")),
                }
            }
        }
        KeyCode::Char('d') => {
            if let Some(id) = app.selected_instance_id() {
                app.confirm_action = Some(ConfirmAction::DeleteInstance(id));
                app.mode = Mode::Confirm;
            }
        }
        KeyCode::PageDown => app.log_scroll = app.log_scroll.saturating_add(10),
        KeyCode::PageUp => app.log_scroll = app.log_scroll.saturating_sub(10),
        _ => {}
    }
}

fn handle_artifacts_key(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Down | KeyCode::Char('j') => {
            if !app.artifacts.is_empty() {
                app.artifact_selected = (app.artifact_selected + 1).min(app.artifacts.len() - 1);
            }
            app.artifact_preview_scroll = 0;
        }
        KeyCode::Up | KeyCode::Char('k') => {
            app.artifact_selected = app.artifact_selected.saturating_sub(1);
            app.artifact_preview_scroll = 0;
        }
        KeyCode::PageDown => {
            app.artifact_preview_scroll = app.artifact_preview_scroll.saturating_add(10)
        }
        KeyCode::PageUp => {
            app.artifact_preview_scroll = app.artifact_preview_scroll.saturating_sub(10)
        }
        KeyCode::Char('r') => app.refresh_artifacts(),
        KeyCode::Char('d') => {
            if let Some(a) = app.artifacts.get(app.artifact_selected) {
                app.confirm_action = Some(ConfirmAction::DeleteArtifact(a.path.clone()));
                app.mode = Mode::Confirm;
            }
        }
        KeyCode::Char('o') => {
            if let Some(a) = app.artifacts.get(app.artifact_selected) {
                let editor = std::env::var("EDITOR").unwrap_or_else(|_| "xdg-open".into());
                let _ = std::process::Command::new(&editor).arg(&a.path).spawn();
                app.toast(format!("Opened with {editor}"));
            }
        }
        KeyCode::Char('e') => {
            if let Some(a) = app.artifacts.get(app.artifact_selected) {
                app.toast(format!("Path: {}", a.path.display()));
            }
        }
        _ => {}
    }
}

fn handle_config_key(app: &mut App, key: KeyEvent) {
    let total = crate::config::KNOWN_ENV_VARS.len();
    match key.code {
        KeyCode::Down | KeyCode::Char('j') => {
            app.config_selected = (app.config_selected + 1).min(total - 1)
        }
        KeyCode::Up | KeyCode::Char('k') => {
            app.config_selected = app.config_selected.saturating_sub(1)
        }
        KeyCode::Enter | KeyCode::Char('e') => {
            let keys = crate::config::KNOWN_ENV_VARS;
            if let Some(k) = keys.get(app.config_selected) {
                app.edit_buffer = app
                    .config
                    .env_overrides
                    .get(*k)
                    .cloned()
                    .unwrap_or_default();
                app.config_editing = true;
            }
        }
        KeyCode::Char('d') => {
            let keys = crate::config::KNOWN_ENV_VARS;
            if let Some(k) = keys.get(app.config_selected) {
                app.config.env_overrides.remove(*k);
                let _ = app.config.save(&app.root);
                app.toast(format!("Cleared {k}"));
            }
        }
        _ => {}
    }
}

fn handle_cdp_key(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Down | KeyCode::Char('j') => {
            app.cdp_method_idx = (app.cdp_method_idx + 1) % crate::cdp::CDP_METHODS.len();
            let (m, _) = crate::cdp::CDP_METHODS[app.cdp_method_idx];
            app.cdp_params_json =
                serde_json::to_string_pretty(&crate::cdp::default_params(m)).unwrap_or("{}".into());
        }
        KeyCode::Up | KeyCode::Char('k') => {
            app.cdp_method_idx = (app.cdp_method_idx + crate::cdp::CDP_METHODS.len() - 1)
                % crate::cdp::CDP_METHODS.len();
            let (m, _) = crate::cdp::CDP_METHODS[app.cdp_method_idx];
            app.cdp_params_json =
                serde_json::to_string_pretty(&crate::cdp::default_params(m)).unwrap_or("{}".into());
        }
        KeyCode::Char('c') => cdp_connect(app),
        KeyCode::Char('x') => {
            app.cdp_client = None;
            app.cdp_connected_to = None;
            app.cdp_response = "Disconnected.".into();
            app.toast("CDP disconnected");
        }
        KeyCode::Char('e') => app.cdp_editing_params = true,
        KeyCode::Char('t') => cdp_refresh_targets(app),
        KeyCode::Enter => cdp_send(app),
        _ => {}
    }
}

fn selected_serve(app: &App) -> Option<(uuid::Uuid, String, u16)> {
    // prefer selected instance if it's a running serve, else first running serve
    if let Some(id) = app.selected_instance_id()
        && let Some(inst) = app.manager.get(id)
        && inst.state == crate::instance::InstanceState::Running
        && let crate::cli::TaskParams::Serve(p) = &inst.params
    {
        return Some((id, p.host.clone(), p.port.parse().unwrap_or(9222)));
    }
    let serves = app.manager.running_serve_instances();
    if let Some(inst) = serves.first()
        && let crate::cli::TaskParams::Serve(p) = &inst.params
    {
        return Some((inst.id, p.host.clone(), p.port.parse().unwrap_or(9222)));
    }
    None
}

fn cdp_connect(app: &mut App) {
    let Some((id, host, port)) = selected_serve(app) else {
        app.toast("No running serve instance — spawn one with [v] in Tasks");
        app.cdp_response = "No running serve instance. Spawn a serve task first (Tasks → v). Dead instances cannot be reattached.".into();
        return;
    };
    let token = app
        .config
        .env_overrides
        .get("OBSCURA_CDP_TOKEN")
        .map(|s| s.as_str());
    // connect in blocking call (fast local)
    match crate::cdp::CdpClient::connect(&host, port, token) {
        Ok(mut client) => match client.get_targets() {
            Ok(targets) => {
                app.cdp_targets = targets;
                app.cdp_client = Some(client);
                app.cdp_connected_to = Some(id);
                app.cdp_response = format!(
                    "Connected to {host}:{port} — {} targets",
                    app.cdp_targets.len()
                );
                app.cdp_events
                    .push(format!("[cdp] connected {host}:{port}"));
                app.toast("CDP connected");
            }
            Err(e) => {
                app.cdp_client = Some(client);
                app.cdp_connected_to = Some(id);
                app.cdp_response = format!("Connected but getTargets failed: {e}");
            }
        },
        Err(e) => {
            app.cdp_response = format!("CDP connect failed ws://{host}:{port} — {e}");
            app.toast("CDP connect failed");
        }
    }
}

fn cdp_refresh_targets(app: &mut App) {
    if app.cdp_client.is_none() {
        cdp_connect(app);
        return;
    }
    // need to take client temporarily
    let mut client = app.cdp_client.take().unwrap();
    match client.get_targets() {
        Ok(t) => {
            app.cdp_targets = t;
            app.cdp_response = format!("{} targets", app.cdp_targets.len());
        }
        Err(e) => app.cdp_response = format!("getTargets failed: {e}"),
    }
    app.cdp_client = Some(client);
}

fn cdp_send(app: &mut App) {
    if app.cdp_client.is_none() {
        app.toast("Not connected — press c");
        return;
    }
    let (method, _) = crate::cdp::CDP_METHODS[app.cdp_method_idx];
    let params: serde_json::Value =
        serde_json::from_str(&app.cdp_params_json).unwrap_or(serde_json::json!({}));
    let mut client = app.cdp_client.take().unwrap();
    // Special handling: screenshot saves to artifacts
    let result = client.call(method, params);
    match result {
        Ok(v) => {
            let pretty = serde_json::to_string_pretty(&v).unwrap_or_else(|_| format!("{v:?}"));
            // screenshot handling
            if method == "Page.captureScreenshot" {
                if let Some(data) = v.get("data").and_then(|d| d.as_str()) {
                    use std::io::Write;
                    // base64 decode minimal (no external crate): use simple decode via python? implement manual
                    if let Some(bytes) = base64_decode(data) {
                        let name = format!("{}_cdp.png", crate::config::timestamp_slug());
                        let path = crate::config::artifacts_dir(&app.root)
                            .join("screenshots")
                            .join(&name);
                        if let Ok(mut f) = std::fs::File::create(&path) {
                            let _ = f.write_all(&bytes);
                            app.cdp_response =
                                format!("Screenshot saved to {}\n{}", path.display(), pretty);
                            app.refresh_artifacts();
                        } else {
                            app.cdp_response = pretty;
                        }
                    } else {
                        app.cdp_response = pretty;
                    }
                } else {
                    app.cdp_response = pretty;
                }
            } else if method == "Page.printToPDF" {
                if let Some(data) = v.get("data").and_then(|d| d.as_str()) {
                    if let Some(bytes) = base64_decode(data) {
                        let name = format!("{}_cdp.pdf", crate::config::timestamp_slug());
                        let path = crate::config::artifacts_dir(&app.root)
                            .join("pdfs")
                            .join(name);
                        let _ = std::fs::write(&path, bytes);
                        app.cdp_response = format!("PDF saved to {}\n{}", path.display(), pretty);
                        app.refresh_artifacts();
                    } else {
                        app.cdp_response = pretty;
                    }
                } else {
                    app.cdp_response = pretty;
                }
            } else {
                app.cdp_response = pretty;
            }
            app.cdp_events.push(format!("[cdp] {method} ok"));
            if method == "Target.getTargets" {
                if let Ok(t) = serde_json::from_value::<Vec<serde_json::Value>>(
                    v.get("targetInfos")
                        .cloned()
                        .unwrap_or(serde_json::json!([])),
                ) {
                    let _ = t;
                }
                // refresh targets list via client
                if let Ok(targets) = client.get_targets() {
                    app.cdp_targets = targets;
                }
            }
        }
        Err(e) => {
            app.cdp_response = format!("Error: {e}");
            app.cdp_events.push(format!("[cdp] {method} error: {e}"));
        }
    }
    app.cdp_client = Some(client);
}

fn base64_decode(input: &str) -> Option<Vec<u8>> {
    // minimal base64 decoder
    const TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut vals: Vec<u8> = Vec::new();
    for c in input.bytes() {
        if c == b'=' {
            break;
        }
        if let Some(pos) = TABLE.iter().position(|&x| x == c) {
            vals.push(pos as u8);
        } else if c == b'\n' || c == b'\r' || c == b' ' {
            continue;
        } else {
            return None;
        }
    }
    let mut out = Vec::new();
    for chunk in vals.chunks(4) {
        if chunk.len() >= 2 {
            out.push((chunk[0] << 2) | (chunk[1] >> 4));
        }
        if chunk.len() >= 3 {
            out.push(((chunk[1] & 0x0f) << 4) | (chunk[2] >> 2));
        }
        if chunk.len() >= 4 {
            out.push(((chunk[2] & 0x03) << 6) | chunk[3]);
        }
    }
    Some(out)
}
