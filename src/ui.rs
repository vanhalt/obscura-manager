use crate::app::{App, Mode, Tab};
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, Borders, Cell, Clear, List, ListItem, Paragraph, Row, Table, Tabs, Wrap,
};

fn header_style() -> Style {
    Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD)
}
fn focus_style() -> Style {
    Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD)
}
fn dim() -> Style {
    Style::default().fg(Color::DarkGray)
}

pub fn render(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(f.area());

    render_header(f, chunks[0], app);
    render_tabs(f, chunks[1], app);
    match app.tab {
        Tab::Dashboard => render_dashboard(f, chunks[2], app),
        Tab::Tasks => render_tasks(f, chunks[2], app),
        Tab::Cdp => render_cdp(f, chunks[2], app),
        Tab::Artifacts => render_artifacts(f, chunks[2], app),
        Tab::Config => render_config(f, chunks[2], app),
        Tab::Logs => render_logs(f, chunks[2], app),
        Tab::Help => render_help(f, chunks[2], app),
    }
    render_footer(f, chunks[3], app);

    if app.mode == Mode::CreateForm || app.mode == Mode::EditingField {
        render_form(f, app);
    }
    if app.mode == Mode::Confirm {
        render_confirm(f, app);
    }
    if app.mode == Mode::Filter {
        render_filter(f, app);
    }
    if app.cdp_editing_params {
        render_cdp_editor(f, app);
    }
    if app.config_editing {
        render_config_editor(f, app);
    }
}

fn render_header(f: &mut Frame, area: Rect, app: &App) {
    let running = app
        .manager
        .instances
        .iter()
        .filter(|i| i.state == crate::instance::InstanceState::Running)
        .count();
    let total = app.manager.instances.len();
    let text = vec![Line::from(vec![
        Span::styled(" OBSCURA MANAGER ", header_style()),
        Span::raw(format!("  {}  ", app.version)),
        Span::styled(format!(" root: {} ", app.root.display()), dim()),
        Span::raw(format!(
            "  instances: {running} running / {total} total  bin: {}",
            app.binary
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "not found".into())
        )),
    ])];
    let p = Paragraph::new(text).block(
        Block::default()
            .borders(Borders::ALL)
            .title(format!("Obscura Manager — ▶ {}", app.tab.label())),
    );
    f.render_widget(p, area);
}

fn render_tabs(f: &mut Frame, area: Rect, app: &App) {
    // No full Block here: a bordered block in a 3-row strip would leave the
    // titles only one row, and in a 2-row strip zero rows (the old bug —
    // tabs changed the body but no tab label was ever visible).
    let titles: Vec<Line> = Tab::all()
        .iter()
        .enumerate()
        .map(|(i, t)| {
            if *t == app.tab {
                Line::from(Span::styled(
                    format!(" ▶ {}:{} ", i + 1, t.label()),
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ))
            } else {
                Line::from(Span::styled(format!(" {}:{} ", i + 1, t.label()), dim()))
            }
        })
        .collect();
    let tabs = Tabs::new(titles)
        .select(app.tab.index())
        .highlight_style(
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .divider("│");
    // Split strip: row 0 = tab titles, row 1 = current-section banner.
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(area);
    f.render_widget(tabs, rows[0]);
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                format!(" ▶ SECTION: {} ", app.tab.label().to_uppercase()),
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(
                    " ({}/{} — key {} jumps here, Tab = next section) ",
                    app.tab.index() + 1,
                    Tab::all().len(),
                    app.tab.index() + 1
                ),
                dim(),
            ),
        ])),
        rows[1],
    );
}

fn render_footer(f: &mut Frame, area: Rect, app: &App) {
    let toast = app
        .toast
        .as_ref()
        .filter(|(_, t)| t.elapsed() < std::time::Duration::from_secs(4))
        .map(|(m, _)| format!("  ★ {m}"))
        .unwrap_or_default();
    let help = match app.tab {
        Tab::Tasks => {
            "n:new f:fetch v:serve p:scrape m:mcp  j/k:nav  s:stop  r:restart(new)  d:delete  /:filter  Tab:next tab  q:quit  ?:help"
        }
        Tab::Artifacts => "j/k:nav  o:open  d:delete  e:path  r:refresh  PgUp/PgDn:scroll preview",
        Tab::Cdp => {
            "c:connect  x:disconnect  t:targets  j/k:method  e:edit params  Enter:send  (pick running serve in Tasks first)"
        }
        Tab::Config => "j/k:nav  Enter/e:edit  d:clear  (saved to config.json + config.toml)",
        Tab::Logs => "j/k:select instance  PgUp/PgDn:scroll logs",
        _ => "1-7:tabs  Tab:next  n:new task (in Tasks)  q:quit  ?:help",
    };
    let line = Line::from(vec![
        Span::styled(help, dim()),
        Span::styled(toast, Style::default().fg(Color::Green)),
    ]);
    f.render_widget(Paragraph::new(line), area);
}

fn instance_rows(app: &App) -> Vec<Row<'static>> {
    let filtered = app.filtered_instances();
    filtered
        .iter()
        .enumerate()
        .map(|(sel_idx, &real_idx)| {
            let i = &app.manager.instances[real_idx];
            let style = if sel_idx == app.selected {
                focus_style()
            } else {
                Style::default()
            };
            let state_style = match i.state {
                crate::instance::InstanceState::Running => Style::default().fg(Color::Green),
                crate::instance::InstanceState::Failed => Style::default().fg(Color::Red),
                crate::instance::InstanceState::Killed => Style::default().fg(Color::Magenta),
                crate::instance::InstanceState::Exited => Style::default().fg(Color::DarkGray),
                crate::instance::InstanceState::Spawning => Style::default().fg(Color::Yellow),
            };
            Row::new(vec![
                Cell::from(i.id.to_string()[..8].to_string()),
                Cell::from(i.params.kind().label().to_string()),
                Cell::from(i.params.target_label()),
                Cell::from(Span::styled(i.state.label().to_string(), state_style)),
                Cell::from(
                    i.params
                        .port()
                        .map(|p| p.to_string())
                        .unwrap_or_else(|| "-".into()),
                ),
                Cell::from(i.pid.map(|p| p.to_string()).unwrap_or_else(|| "-".into())),
                Cell::from(i.started_at.chars().take(19).collect::<String>()),
            ])
            .style(style)
        })
        .collect()
}

fn render_dashboard(f: &mut Frame, area: Rect, app: &App) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(area);
    // left: instance table
    let rows = instance_rows(app);
    let widths = [
        Constraint::Length(9),
        Constraint::Length(7),
        Constraint::Min(20),
        Constraint::Length(10),
        Constraint::Length(6),
        Constraint::Length(8),
        Constraint::Length(19),
    ];
    let table = Table::new(rows, widths)
        .header(
            Row::new(vec![
                "ID", "Kind", "Target", "State", "Port", "PID", "Started",
            ])
            .style(header_style()),
        )
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Instances (Tasks tab for actions)"),
        );
    f.render_widget(table, cols[0]);

    // right: stats + root
    let running = app
        .manager
        .instances
        .iter()
        .filter(|i| i.state == crate::instance::InstanceState::Running)
        .count();
    let failed = app
        .manager
        .instances
        .iter()
        .filter(|i| i.state == crate::instance::InstanceState::Failed)
        .count();
    let mut lines = vec![
        Line::from(vec![
            Span::styled("Version: ", header_style()),
            Span::raw(app.version.clone()),
        ]),
        Line::from(vec![
            Span::styled("Binary: ", header_style()),
            Span::raw(
                app.binary
                    .as_ref()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| "NOT FOUND".into()),
            ),
        ]),
        Line::from(vec![
            Span::styled("OBSCURA_ROOT: ", header_style()),
            Span::raw(app.root.display().to_string()),
        ]),
        Line::from(vec![
            Span::styled("Artifacts: ", header_style()),
            Span::raw(
                crate::config::artifacts_dir(&app.root)
                    .display()
                    .to_string(),
            ),
        ]),
        Line::from(""),
        Line::from(format!(
            "Running: {running}  Failed: {failed}  Total: {}",
            app.manager.instances.len()
        )),
        Line::from(format!("Artifacts on disk: {}", app.artifacts.len())),
        Line::from(""),
        Line::from(Span::styled("Quick start:", header_style())),
        Line::from("  Tasks → v  spawn serve (CDP on 9222)"),
        Line::from("  Tasks → f  spawn fetch"),
        Line::from("  CDP → c    connect to running serve"),
        Line::from("  Dead instances stay dead — press r to spawn a new one."),
    ];
    if app.binary.is_none() {
        lines.push(Line::from(Span::styled(
            "⚠ obscura not found — install from github.com/h4ckf0r0day/obscura or set OBSCURA_BIN",
            Style::default().fg(Color::Red),
        )));
    }
    let p = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).title("Overview"))
        .wrap(Wrap { trim: false });
    f.render_widget(p, cols[1]);
}

fn render_tasks(f: &mut Frame, area: Rect, app: &App) {
    let rows_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(area);
    let rows = instance_rows(app);
    let widths = [
        Constraint::Length(9),
        Constraint::Length(7),
        Constraint::Min(20),
        Constraint::Length(10),
        Constraint::Length(6),
        Constraint::Length(8),
        Constraint::Length(19),
    ];
    let title = if app.filter.is_empty() {
        "Instances — n:new  f:fetch v:serve p:scrape m:mcp  s:stop r:restart(new) d:delete"
            .to_string()
    } else {
        format!("Instances — filter: '{}'  (/:edit, Esc:done)", app.filter)
    };
    let table = Table::new(rows, widths)
        .header(
            Row::new(vec![
                "ID", "Kind", "Target", "State", "Port", "PID", "Started",
            ])
            .style(header_style()),
        )
        .block(Block::default().borders(Borders::ALL).title(title));
    f.render_widget(table, rows_layout[0]);

    // detail
    let detail = if let Some(id) = app.selected_instance_id() {
        if let Some(inst) = app.manager.get(id) {
            let mut lines = vec![
                Line::from(vec![
                    Span::styled("ID: ", header_style()),
                    Span::raw(inst.id.to_string()),
                ]),
                Line::from(vec![
                    Span::styled("Kind: ", header_style()),
                    Span::raw(inst.params.kind().label()),
                ]),
                Line::from(vec![
                    Span::styled("Argv: ", header_style()),
                    Span::raw(format!("obscura {}", inst.argv.join(" "))),
                ]),
                Line::from(vec![
                    Span::styled("State: ", header_style()),
                    Span::raw(format!(
                        "{:?} pid={:?} exit={:?}",
                        inst.state, inst.pid, inst.exit_code
                    )),
                ]),
            ];
            if let Some(n) = &inst.exit_note {
                lines.push(Line::from(vec![
                    Span::styled("Note: ", header_style()),
                    Span::raw(n.clone()),
                ]));
            }
            lines.push(Line::from(vec![
                Span::styled("Logs: ", header_style()),
                Span::raw(format!(
                    "{} | {}",
                    inst.stdout_log.display(),
                    inst.stderr_log.display()
                )),
            ]));
            if !inst.artifact_paths.is_empty() {
                lines.push(Line::from(vec![
                    Span::styled("Artifacts: ", header_style()),
                    Span::raw(
                        inst.artifact_paths
                            .iter()
                            .map(|p| p.display().to_string())
                            .collect::<Vec<_>>()
                            .join(", "),
                    ),
                ]));
            }
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled("— output (tail) —", dim())));
            let logs = app.manager.logs.get(&id).cloned().unwrap_or_default();
            for l in logs.iter().rev().take(12).rev() {
                lines.push(Line::from(l.clone()));
            }
            // Show the actual dump file content here: with --output set, the
            // dump goes to the file and stdout is empty, so without this the
            // Tasks view looked like the dump produced nothing.
            for path in &inst.artifact_paths {
                if path.exists() {
                    lines.push(Line::from(""));
                    lines.push(Line::from(Span::styled(
                        format!("— dump: {} —", path.display()),
                        header_style(),
                    )));
                    let preview = crate::artifacts::preview(path, 3000);
                    for l in preview.lines().take(20) {
                        lines.push(Line::from(l.to_string()));
                    }
                } else if inst.state.is_terminal() {
                    lines.push(Line::from(Span::styled(
                        format!("— dump missing (not created): {} —", path.display()),
                        Style::default().fg(Color::Red),
                    )));
                }
            }
            lines
        } else {
            vec![Line::from("No instance selected")]
        }
    } else {
        vec![Line::from(
            "No instances yet. Press n / f / v / p / m to spawn a task.",
        )]
    };
    let p = Paragraph::new(detail)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Detail — PgUp/PgDn scroll in Logs tab"),
        )
        .wrap(Wrap { trim: false })
        .scroll((app.log_scroll, 0));
    f.render_widget(p, rows_layout[1]);
}

fn render_cdp(f: &mut Frame, area: Rect, app: &App) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(28),
            Constraint::Percentage(36),
            Constraint::Percentage(36),
        ])
        .split(area);

    // left: targets + methods
    let left_rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(35), Constraint::Percentage(65)])
        .split(cols[0]);
    let target_items: Vec<ListItem> = if app.cdp_targets.is_empty() {
        vec![ListItem::new(
            "No targets — press c to connect, t to refresh",
        )]
    } else {
        app.cdp_targets
            .iter()
            .map(|t| ListItem::new(format!("{} {} {}", t.kind, t.title, t.url)))
            .collect()
    };
    let conn_title = match &app.cdp_connected_to {
        Some(id) => format!("Targets — connected {}", &id.to_string()[..8]),
        None => "Targets — NOT CONNECTED".to_string(),
    };
    f.render_widget(
        List::new(target_items).block(Block::default().borders(Borders::ALL).title(conn_title)),
        left_rows[0],
    );

    let method_items: Vec<ListItem> = crate::cdp::CDP_METHODS
        .iter()
        .enumerate()
        .map(|(i, (m, desc))| {
            let style = if i == app.cdp_method_idx {
                focus_style()
            } else {
                Style::default()
            };
            ListItem::new(Line::from(vec![
                Span::styled(format!("{m} "), style),
                Span::styled(format!("— {desc}"), dim()),
            ]))
        })
        .collect();
    f.render_widget(
        List::new(method_items).block(
            Block::default()
                .borders(Borders::ALL)
                .title("CDP Methods (j/k, Enter:send, e:edit params)"),
        ),
        left_rows[1],
    );

    // middle: params + response
    let mid_rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(cols[1]);
    let (method, _) = crate::cdp::CDP_METHODS[app.cdp_method_idx];
    let params_p = Paragraph::new(app.cdp_params_json.clone())
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!("Params — {method} (e:edit)")),
        )
        .wrap(Wrap { trim: false });
    f.render_widget(params_p, mid_rows[0]);
    let resp = Paragraph::new(app.cdp_response.clone())
        .block(Block::default().borders(Borders::ALL).title("Response"))
        .wrap(Wrap { trim: false });
    f.render_widget(resp, mid_rows[1]);

    // right: events + MCP tools reference + help
    let right_rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(cols[2]);
    let events: Vec<ListItem> = if app.cdp_events.is_empty() {
        vec![ListItem::new("No events yet")]
    } else {
        app.cdp_events
            .iter()
            .rev()
            .take(30)
            .map(|e| ListItem::new(e.clone()))
            .collect()
    };
    f.render_widget(
        List::new(events).block(
            Block::default()
                .borders(Borders::ALL)
                .title("CDP Events / Log"),
        ),
        right_rows[0],
    );

    let mcp_items: Vec<ListItem> = crate::cdp::MCP_TOOLS
        .iter()
        .map(|(n, d)| ListItem::new(format!("{n} — {d}")))
        .collect();
    f.render_widget(
        List::new(mcp_items).block(
            Block::default()
                .borders(Borders::ALL)
                .title("MCP Tools (spawn mcp task in Tasks → m)"),
        ),
        right_rows[1],
    );
}

fn render_artifacts(f: &mut Frame, area: Rect, app: &App) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
        .split(area);
    let items: Vec<ListItem> = if app.artifacts.is_empty() {
        vec![ListItem::new(
            "No artifacts yet — run a fetch/scrape or CDP screenshot",
        )]
    } else {
        app.artifacts
            .iter()
            .enumerate()
            .map(|(i, a)| {
                let style = if i == app.artifact_selected {
                    focus_style()
                } else {
                    Style::default()
                };
                ListItem::new(Line::from(vec![
                    Span::styled(format!("[{}] ", a.kind), dim()),
                    Span::raw(
                        a.path
                            .file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_default(),
                    ),
                    Span::styled(
                        format!("  {}  {}", crate::artifacts::human_size(a.size), a.modified),
                        dim(),
                    ),
                ]))
                .style(style)
            })
            .collect()
    };
    f.render_widget(
        List::new(items).block(Block::default().borders(Borders::ALL).title(format!(
            "Artifacts — {} (j/k, o:open d:delete e:path r:refresh)",
            crate::config::artifacts_dir(&app.root).display()
        ))),
        cols[0],
    );

    let preview = if let Some(a) = app.artifacts.get(app.artifact_selected) {
        crate::artifacts::preview(&a.path, 8000)
    } else {
        "Select an artifact".into()
    };
    let title = app
        .artifacts
        .get(app.artifact_selected)
        .map(|a| format!("Preview — {}", a.path.display()))
        .unwrap_or_else(|| "Preview".into());
    let p = Paragraph::new(preview)
        .block(Block::default().borders(Borders::ALL).title(title))
        .wrap(Wrap { trim: false })
        .scroll((app.artifact_preview_scroll, 0));
    f.render_widget(p, cols[1]);
}

fn render_config(f: &mut Frame, area: Rect, app: &App) {
    let items: Vec<ListItem> = crate::config::KNOWN_ENV_VARS
        .iter()
        .enumerate()
        .map(|(i, k)| {
            let v = app
                .config
                .env_overrides
                .get(*k)
                .cloned()
                .unwrap_or_default();
            let style = if i == app.config_selected {
                focus_style()
            } else {
                Style::default()
            };
            let display = if v.is_empty() {
                "(unset)".to_string()
            } else {
                v
            };
            ListItem::new(Line::from(vec![
                Span::styled(format!("{k} = "), style),
                Span::raw(display),
            ]))
        })
        .collect();
    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title("Config — OBSCURA_* env (Enter/e:edit, d:clear) — applied to new spawns, saved to config.json"),
    );
    f.render_widget(list, area);
}

fn render_logs(f: &mut Frame, area: Rect, app: &App) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(35), Constraint::Percentage(65)])
        .split(area);
    let rows = instance_rows(app);
    let widths = [
        Constraint::Length(9),
        Constraint::Length(7),
        Constraint::Min(14),
        Constraint::Length(10),
        Constraint::Length(6),
        Constraint::Length(8),
        Constraint::Length(19),
    ];
    let table = Table::new(rows, widths)
        .header(
            Row::new(vec![
                "ID", "Kind", "Target", "State", "Port", "PID", "Started",
            ])
            .style(header_style()),
        )
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Select instance (j/k)"),
        );
    f.render_widget(table, cols[0]);

    let text = if let Some(id) = app.selected_instance_id() {
        let logs = app.manager.logs.get(&id).cloned().unwrap_or_default();
        if logs.is_empty() {
            // try read log files
            if let Some(inst) = app.manager.get(id) {
                let mut s = String::new();
                if let Ok(c) = std::fs::read_to_string(&inst.stdout_log) {
                    s.push_str(&c);
                }
                if let Ok(c) = std::fs::read_to_string(&inst.stderr_log)
                    && !c.is_empty()
                {
                    s.push_str("\n[stderr file]\n");
                    s.push_str(&c);
                }
                if s.is_empty() {
                    "No logs yet".into()
                } else {
                    s
                }
            } else {
                "No logs".into()
            }
        } else {
            logs.join("\n")
        }
    } else {
        "No instance selected".into()
    };
    let p = Paragraph::new(text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Logs (PgUp/PgDn scroll)"),
        )
        .wrap(Wrap { trim: false })
        .scroll((app.log_scroll, 0));
    f.render_widget(p, cols[1]);
}

fn render_help(f: &mut Frame, area: Rect, _app: &App) {
    let text = vec![
        Line::from(Span::styled("Obscura Manager — Help", header_style())),
        Line::from(""),
        Line::from("Global:  q quit  Ctrl+C quit  ? help  1-7 jump tab  Tab/Shift+Tab cycle tabs"),
        Line::from(
            "Tasks:   n new (fetch)  f fetch  v serve  p scrape  m mcp  j/k navigate  Enter detail",
        ),
        Line::from(
            "         s stop (confirm)  r restart as NEW instance (dead stays dead, no reattach)",
        ),
        Line::from("         d delete history (confirm)  / filter  Esc clear/close"),
        Line::from(
            "Form:    Tab/↑↓ next field  Enter/Space cycle bool/select or edit text  e edit text",
        ),
        Line::from(
            "         1 fetch 2 serve 3 scrape 4 mcp (switch kind)  F5 or Shift+S spawn  Esc cancel",
        ),
        Line::from(
            "CDP:     c connect to running serve  x disconnect  t refresh targets  j/k choose method",
        ),
        Line::from(
            "         e edit JSON params  Enter send  — screenshot/PDF auto-save to artifacts/",
        ),
        Line::from(
            "Artifacts: j/k navigate  o open ($EDITOR/xdg-open)  d delete (confirm)  e show path  r refresh",
        ),
        Line::from("Config:  j/k navigate  Enter/e edit env var  d clear  (applies to next spawn)"),
        Line::from("Logs:    j/k select instance  PgUp/PgDn scroll"),
        Line::from(""),
        Line::from("No reattach: once Exited/Failed/Killed, an instance is dead forever."),
        Line::from(
            "Press r on it to spawn a fresh instance with the same parameters (new ID/PID).",
        ),
        Line::from("Same after manager restart — history is for inspection only."),
        Line::from(""),
        Line::from(
            "OBSCURA_ROOT layout: artifacts/{screenshots,dumps,scrapes,pdfs,exports,logs} + instances.json + config.json",
        ),
        Line::from(
            "Binary: OBSCURA_BIN > PATH > $OBSCURA_ROOT/bin/obscura  (verified: /home/vht/.local/bin/obscura)",
        ),
    ];
    let p = Paragraph::new(text)
        .block(Block::default().borders(Borders::ALL).title("Help (?)"))
        .wrap(Wrap { trim: false });
    f.render_widget(p, area);
}

fn centered(area: Rect, pct_x: u16, pct_y: u16) -> Rect {
    let vert = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - pct_y) / 2),
            Constraint::Percentage(pct_y),
            Constraint::Percentage((100 - pct_y) / 2),
        ])
        .split(area);
    let horiz = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - pct_x) / 2),
            Constraint::Percentage(pct_x),
            Constraint::Percentage((100 - pct_x) / 2),
        ])
        .split(vert[1]);
    horiz[1]
}

fn render_form(f: &mut Frame, app: &App) {
    let area = centered(f.area(), 70, 80);
    f.render_widget(Clear, area);
    let title = format!(
        "New Task — {}  [1:fetch 2:serve 3:scrape 4:mcp]  F5/Shift+S:spawn  Esc:cancel",
        app.form_kind.label()
    );
    let items: Vec<ListItem> = app
        .form_fields
        .iter()
        .enumerate()
        .map(|(i, field)| {
            let focused = i == app.form_focus;
            let marker = if focused { "▶ " } else { "  " };
            let style = if focused {
                focus_style()
            } else {
                Style::default()
            };
            let val = if app.mode == Mode::EditingField && focused {
                format!("{}▊", app.edit_buffer)
            } else {
                field.value.clone()
            };
            ListItem::new(Line::from(vec![
                Span::styled(format!("{marker}{}: ", field.label), style),
                Span::raw(val),
            ]))
        })
        .collect();
    f.render_widget(
        List::new(items).block(Block::default().borders(Borders::ALL).title(title)),
        area,
    );
}

fn render_confirm(f: &mut Frame, app: &App) {
    let area = centered(f.area(), 50, 20);
    f.render_widget(Clear, area);
    let msg = match &app.confirm_action {
        Some(crate::app::ConfirmAction::Kill(id)) => format!(
            "Kill instance {}?  [y/Enter] yes  [n/Esc] no",
            &id.to_string()[..8]
        ),
        Some(crate::app::ConfirmAction::DeleteInstance(id)) => format!(
            "Delete instance {} from history?  [y] yes  [n] no",
            &id.to_string()[..8]
        ),
        Some(crate::app::ConfirmAction::DeleteArtifact(p)) => {
            format!("Delete {}?  [y] yes  [n] no", p.display())
        }
        None => "Confirm?".into(),
    };
    let p = Paragraph::new(msg)
        .block(Block::default().borders(Borders::ALL).title("Confirm"))
        .wrap(Wrap { trim: true });
    f.render_widget(p, area);
}

fn render_filter(f: &mut Frame, app: &App) {
    let area = centered(f.area(), 60, 15);
    f.render_widget(Clear, area);
    let p = Paragraph::new(format!("Filter: {}▊  (Enter/Esc done)", app.filter)).block(
        Block::default()
            .borders(Borders::ALL)
            .title("Filter instances"),
    );
    f.render_widget(p, area);
}

fn render_cdp_editor(f: &mut Frame, app: &App) {
    let area = centered(f.area(), 60, 40);
    f.render_widget(Clear, area);
    let p = Paragraph::new(format!("{}▊\n\nEnter/Esc done", app.cdp_params_json))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Edit CDP params JSON"),
        )
        .wrap(Wrap { trim: false });
    f.render_widget(p, area);
}

fn render_config_editor(f: &mut Frame, app: &App) {
    let area = centered(f.area(), 60, 20);
    f.render_widget(Clear, area);
    let key = crate::config::KNOWN_ENV_VARS
        .get(app.config_selected)
        .copied()
        .unwrap_or("");
    let p = Paragraph::new(format!(
        "{key} = {}▊\n\nEnter save  Esc cancel",
        app.edit_buffer
    ))
    .block(Block::default().borders(Borders::ALL).title("Edit env var"))
    .wrap(Wrap { trim: false });
    f.render_widget(p, area);
}
