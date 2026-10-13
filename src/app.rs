use crate::artifacts::Artifact;
use crate::cdp::CdpClient;
use crate::cli::*;
use crate::config::ManagerConfig;
use crate::instance::{Manager, ProcEvent};
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Dashboard,
    Tasks,
    Cdp,
    Artifacts,
    Config,
    Logs,
    Help,
}
impl Tab {
    pub fn all() -> [Self; 7] {
        [
            Self::Dashboard,
            Self::Tasks,
            Self::Cdp,
            Self::Artifacts,
            Self::Config,
            Self::Logs,
            Self::Help,
        ]
    }
    pub fn label(&self) -> &'static str {
        match self {
            Self::Dashboard => "Dashboard",
            Self::Tasks => "Tasks",
            Self::Cdp => "CDP",
            Self::Artifacts => "Artifacts",
            Self::Config => "Config",
            Self::Logs => "Logs",
            Self::Help => "Help",
        }
    }
    pub fn index(&self) -> usize {
        Self::all().iter().position(|t| t == self).unwrap_or(0)
    }
    pub fn from_index(i: usize) -> Self {
        Self::all()[i % 7]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Normal,
    CreateForm,
    EditingField,
    Confirm,
    Filter,
}

#[derive(Debug, Clone)]
pub struct FormField {
    pub key: String,
    pub label: String,
    pub value: String,
    pub is_bool: bool,
    pub options: Vec<String>,
}
impl FormField {
    pub fn text(key: &str, label: &str, value: &str) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
            value: value.into(),
            is_bool: false,
            options: Vec::new(),
        }
    }
    pub fn bool_field(key: &str, label: &str, value: bool) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
            value: if value { "true".into() } else { "false".into() },
            is_bool: true,
            options: Vec::new(),
        }
    }
    pub fn select(key: &str, label: &str, value: &str, options: &[&str]) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
            value: value.into(),
            is_bool: false,
            options: options.iter().map(|s| s.to_string()).collect(),
        }
    }
    pub fn cycle(&mut self) {
        if self.is_bool {
            self.value = if self.value == "true" {
                "false".into()
            } else {
                "true".into()
            };
        } else if !self.options.is_empty() {
            let idx = self
                .options
                .iter()
                .position(|o| o == &self.value)
                .unwrap_or(0);
            self.value = self.options[(idx + 1) % self.options.len()].clone();
        }
    }
    pub fn as_bool(&self) -> bool {
        self.value == "true"
    }
}

pub fn fetch_form(p: &FetchParams) -> Vec<FormField> {
    vec![
        FormField::text("url", "URL", &p.url),
        FormField::text("file", "Batch file (--file, - for stdin)", &p.file),
        FormField::select(
            "dump",
            "Dump format",
            &p.dump,
            &[
                "html", "text", "links", "markdown", "original", "assets", "cookies",
            ],
        ),
        FormField::text("selector", "Selector (CSS)", &p.selector),
        FormField::text("eval", "Eval JS", &p.eval),
        FormField::text("output", "Output file (blank=artifacts)", &p.output),
        FormField::text("screenshot", "Screenshot PNG (blank=none)", &p.screenshot),
        FormField::text("wait", "Wait seconds", &p.wait),
        FormField::text("timeout", "Timeout seconds", &p.timeout),
        FormField::select(
            "wait_until",
            "Wait until",
            &p.wait_until,
            &["load", "domcontentloaded", "networkidle0", "networkidle2"],
        ),
        FormField::text("concurrency", "Batch concurrency", &p.concurrency),
        FormField::bool_field("quiet", "Quiet", p.quiet),
        FormField::text("proxy", "Proxy", &p.global.proxy),
        FormField::bool_field("stealth", "Stealth", p.global.stealth),
        FormField::bool_field("obey_robots", "Obey robots", p.global.obey_robots),
        FormField::text("user_agent", "User-Agent", &p.global.user_agent),
        FormField::text("storage_dir", "Storage dir", &p.global.storage_dir),
        FormField::bool_field(
            "allow_private_network",
            "Allow private network",
            p.global.allow_private_network,
        ),
        FormField::text("v8_flags", "V8 flags", &p.global.v8_flags),
        FormField::bool_field("verbose", "Verbose", p.global.verbose),
    ]
}
pub fn serve_form(p: &ServeParams) -> Vec<FormField> {
    vec![
        FormField::text("port", "Port", &p.port),
        FormField::text("host", "Host", &p.host),
        FormField::text("workers", "Workers", &p.workers),
        FormField::text("max_connections", "Max connections", &p.max_connections),
        FormField::text("ready_file", "Ready file", &p.ready_file),
        FormField::text("font_dirs", "Font dirs (comma-sep)", &p.font_dirs.join(",")),
        FormField::bool_field(
            "allow_file_access",
            "Allow file access",
            p.allow_file_access,
        ),
        FormField::bool_field("quiet", "Quiet", p.quiet),
        FormField::text("proxy", "Proxy", &p.global.proxy),
        FormField::bool_field("stealth", "Stealth", p.global.stealth),
        FormField::bool_field("obey_robots", "Obey robots", p.global.obey_robots),
        FormField::text("user_agent", "User-Agent", &p.global.user_agent),
        FormField::text("storage_dir", "Storage dir", &p.global.storage_dir),
        FormField::bool_field(
            "allow_private_network",
            "Allow private network",
            p.global.allow_private_network,
        ),
        FormField::text("v8_flags", "V8 flags", &p.global.v8_flags),
        FormField::bool_field("verbose", "Verbose", p.global.verbose),
    ]
}
pub fn scrape_form(p: &ScrapeParams) -> Vec<FormField> {
    vec![
        FormField::text("urls", "URLs (comma-sep)", &p.urls.join(",")),
        FormField::text("eval", "Eval JS", &p.eval),
        FormField::text("concurrency", "Concurrency", &p.concurrency),
        FormField::select("format", "Format", &p.format, &["json", "text"]),
        FormField::text("timeout", "Timeout", &p.timeout),
        FormField::bool_field("use_stdin", "Read URLs from stdin (-)", p.use_stdin),
        FormField::bool_field("quiet", "Quiet", p.quiet),
        FormField::text("proxy", "Proxy", &p.global.proxy),
        FormField::bool_field("stealth", "Stealth", p.global.stealth),
        FormField::bool_field("obey_robots", "Obey robots", p.global.obey_robots),
        FormField::text("user_agent", "User-Agent", &p.global.user_agent),
        FormField::text("storage_dir", "Storage dir", &p.global.storage_dir),
        FormField::bool_field(
            "allow_private_network",
            "Allow private network",
            p.global.allow_private_network,
        ),
        FormField::text("v8_flags", "V8 flags", &p.global.v8_flags),
        FormField::bool_field("verbose", "Verbose", p.global.verbose),
    ]
}
pub fn mcp_form(p: &McpParams) -> Vec<FormField> {
    vec![
        FormField::bool_field("http", "HTTP transport", p.http),
        FormField::text("host", "Host (HTTP)", &p.host),
        FormField::text("port", "Port (HTTP)", &p.port),
        FormField::text("proxy", "Proxy", &p.global.proxy),
        FormField::bool_field("stealth", "Stealth", p.global.stealth),
        FormField::bool_field("obey_robots", "Obey robots", p.global.obey_robots),
        FormField::text("user_agent", "User-Agent", &p.global.user_agent),
        FormField::text("storage_dir", "Storage dir", &p.global.storage_dir),
        FormField::bool_field(
            "allow_private_network",
            "Allow private network",
            p.global.allow_private_network,
        ),
        FormField::text("v8_flags", "V8 flags", &p.global.v8_flags),
        FormField::bool_field("verbose", "Verbose", p.global.verbose),
    ]
}

fn get<'a>(fields: &'a [FormField], key: &str) -> &'a str {
    fields
        .iter()
        .find(|f| f.key == key)
        .map(|f| f.value.as_str())
        .unwrap_or("")
}
fn get_bool(fields: &[FormField], key: &str) -> bool {
    fields
        .iter()
        .find(|f| f.key == key)
        .is_some_and(|f| f.as_bool())
}
fn global_from(fields: &[FormField]) -> GlobalOpts {
    GlobalOpts {
        verbose: get_bool(fields, "verbose"),
        proxy: get(fields, "proxy").to_string(),
        stealth: get_bool(fields, "stealth"),
        obey_robots: get_bool(fields, "obey_robots"),
        user_agent: get(fields, "user_agent").to_string(),
        storage_dir: get(fields, "storage_dir").to_string(),
        allow_private_network: get_bool(fields, "allow_private_network"),
        v8_flags: get(fields, "v8_flags").to_string(),
    }
}
pub fn params_from_form(kind: TaskKind, fields: &[FormField]) -> TaskParams {
    match kind {
        TaskKind::Fetch => TaskParams::Fetch(FetchParams {
            url: get(fields, "url").to_string(),
            dump: get(fields, "dump").to_string(),
            file: get(fields, "file").to_string(),
            concurrency: get(fields, "concurrency").to_string(),
            selector: get(fields, "selector").to_string(),
            wait: get(fields, "wait").to_string(),
            timeout: get(fields, "timeout").to_string(),
            wait_until: get(fields, "wait_until").to_string(),
            eval: get(fields, "eval").to_string(),
            output: get(fields, "output").to_string(),
            screenshot: get(fields, "screenshot").to_string(),
            quiet: get_bool(fields, "quiet"),
            global: global_from(fields),
        }),
        TaskKind::Serve => TaskParams::Serve(ServeParams {
            port: get(fields, "port").to_string(),
            host: get(fields, "host").to_string(),
            workers: get(fields, "workers").to_string(),
            max_connections: get(fields, "max_connections").to_string(),
            ready_file: get(fields, "ready_file").to_string(),
            font_dirs: get(fields, "font_dirs")
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect(),
            allow_file_access: get_bool(fields, "allow_file_access"),
            quiet: get_bool(fields, "quiet"),
            global: global_from(fields),
        }),
        TaskKind::Scrape => TaskParams::Scrape(ScrapeParams {
            urls: get(fields, "urls")
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect(),
            eval: get(fields, "eval").to_string(),
            concurrency: get(fields, "concurrency").to_string(),
            format: get(fields, "format").to_string(),
            timeout: get(fields, "timeout").to_string(),
            quiet: get_bool(fields, "quiet"),
            use_stdin: get_bool(fields, "use_stdin"),
            global: global_from(fields),
        }),
        TaskKind::Mcp => TaskParams::Mcp(McpParams {
            http: get_bool(fields, "http"),
            host: get(fields, "host").to_string(),
            port: get(fields, "port").to_string(),
            global: global_from(fields),
        }),
    }
}

pub struct App {
    pub root: PathBuf,
    pub binary: Option<PathBuf>,
    pub version: String,
    pub tab: Tab,
    pub mode: Mode,
    pub manager: Manager,
    pub tx: Sender<ProcEvent>,
    pub rx: Receiver<ProcEvent>,
    pub selected: usize,
    pub form_kind: TaskKind,
    pub form_fields: Vec<FormField>,
    pub form_focus: usize,
    pub edit_buffer: String,
    pub toast: Option<(String, std::time::Instant)>,
    pub confirm_action: Option<ConfirmAction>,
    pub filter: String,
    pub artifacts: Vec<Artifact>,
    pub artifact_selected: usize,
    pub artifact_preview_scroll: u16,
    pub config: ManagerConfig,
    pub config_selected: usize,
    pub config_editing: bool,
    pub log_scroll: u16,
    // CDP
    pub cdp_client: Option<CdpClient>,
    pub cdp_connected_to: Option<Uuid>,
    pub cdp_targets: Vec<crate::cdp::CdpTarget>,
    pub cdp_method_idx: usize,
    pub cdp_params_json: String,
    pub cdp_response: String,
    pub cdp_events: Vec<String>,
    pub cdp_editing_params: bool,
    pub should_quit: bool,
    pub tick: u64,
}

#[derive(Debug, Clone)]
pub enum ConfirmAction {
    Kill(Uuid),
    DeleteInstance(Uuid),
    DeleteArtifact(PathBuf),
}

impl App {
    pub fn new(root: PathBuf) -> Self {
        let _ = crate::config::ensure_layout(&root);
        let binary = crate::config::resolve_binary(&root);
        let version = crate::config::obscura_version(binary.as_deref());
        let manager = Manager::load_history(&root);
        // save rewritten no-reattach history
        manager.save_history(&root);
        let (tx, rx) = std::sync::mpsc::channel();
        let config = ManagerConfig::load(&root);
        let artifacts = crate::artifacts::scan(&root);
        Self {
            root,
            binary,
            version,
            tab: Tab::Dashboard,
            mode: Mode::Normal,
            manager,
            tx,
            rx,
            selected: 0,
            form_kind: TaskKind::Fetch,
            form_fields: fetch_form(&FetchParams::default()),
            form_focus: 0,
            edit_buffer: String::new(),
            toast: None,
            confirm_action: None,
            filter: String::new(),
            artifacts,
            artifact_selected: 0,
            artifact_preview_scroll: 0,
            config,
            config_selected: 0,
            config_editing: false,
            log_scroll: 0,
            cdp_client: None,
            cdp_connected_to: None,
            cdp_targets: Vec::new(),
            cdp_method_idx: 0,
            cdp_params_json: "{}".into(),
            cdp_response:
                "Not connected. Select a running serve instance and press [c] to connect.".into(),
            cdp_events: Vec::new(),
            cdp_editing_params: false,
            should_quit: false,
            tick: 0,
        }
    }
    pub fn toast(&mut self, msg: impl Into<String>) {
        self.toast = Some((msg.into(), std::time::Instant::now()));
    }
    pub fn filtered_instances(&self) -> Vec<usize> {
        self.manager
            .instances
            .iter()
            .enumerate()
            .filter(|(_, i)| {
                if self.filter.is_empty() {
                    true
                } else {
                    let hay = format!(
                        "{} {} {}",
                        i.params.kind().label(),
                        i.params.target_label(),
                        i.state.label()
                    )
                    .to_lowercase();
                    hay.contains(&self.filter.to_lowercase())
                }
            })
            .map(|(idx, _)| idx)
            .collect()
    }
    pub fn selected_instance_id(&self) -> Option<Uuid> {
        let filtered = self.filtered_instances();
        let real_idx = filtered.get(self.selected).copied().or_else(|| {
            if self.manager.instances.is_empty() {
                None
            } else {
                Some(self.selected.min(self.manager.instances.len() - 1))
            }
        })?;
        self.manager.instances.get(real_idx).map(|i| i.id)
    }
    pub fn open_form(&mut self, kind: TaskKind) {
        self.form_kind = kind;
        self.form_fields = match kind {
            TaskKind::Fetch => fetch_form(&FetchParams::default()),
            TaskKind::Serve => serve_form(&ServeParams::default()),
            TaskKind::Scrape => scrape_form(&ScrapeParams::default()),
            TaskKind::Mcp => mcp_form(&McpParams::default()),
        };
        self.form_focus = 0;
        self.mode = Mode::CreateForm;
    }
    pub fn refresh_artifacts(&mut self) {
        self.artifacts = crate::artifacts::scan(&self.root);
        if self.artifact_selected >= self.artifacts.len() && !self.artifacts.is_empty() {
            self.artifact_selected = self.artifacts.len() - 1;
        }
    }
    pub fn on_tick(&mut self) {
        self.tick += 1;
        // drain proc events
        while let Ok(ev) = self.rx.try_recv() {
            self.manager.handle_event(ev);
        }
        let tx = self.tx.clone();
        self.manager.poll(&tx);
        while let Ok(ev) = self.rx.try_recv() {
            self.manager.handle_event(ev);
        }
        if self.tick.is_multiple_of(50) {
            self.manager.save_history(&self.root);
        }
        // Refresh artifacts promptly (every ~0.5s) so a finished fetch's dump
        // appears in Artifacts/Tasks without waiting for the periodic save.
        if self.tick.is_multiple_of(10) {
            self.refresh_artifacts();
        }
        // CDP health: if connected instance died, disconnect (no reattach)
        if let Some(id) = self.cdp_connected_to {
            let alive = self
                .manager
                .get(id)
                .is_some_and(|i| i.state == crate::instance::InstanceState::Running);
            if !alive {
                self.cdp_client = None;
                self.cdp_connected_to = None;
                self.cdp_response =
                    "Disconnected: serve instance died. Spawn a new serve instance (no reattach)."
                        .into();
                self.cdp_events
                    .push("[cdp] disconnected — instance dead".into());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn params_from_form_roundtrip_fetch() {
        let fields = fetch_form(&FetchParams::default());
        let p = params_from_form(TaskKind::Fetch, &fields);
        assert_eq!(p.kind(), TaskKind::Fetch);
    }
    #[test]
    fn render_dashboard_small_terminal_no_panic() {
        let root = std::env::temp_dir().join(format!("obscura-ui-{}", uuid::Uuid::new_v4()));
        let app = App::new(root.clone());
        let backend = ratatui::backend::TestBackend::new(80, 24);
        let mut term = ratatui::Terminal::new(backend).unwrap();
        term.draw(|f| crate::ui::render(f, &app)).unwrap();
        let backend2 = ratatui::backend::TestBackend::new(160, 50);
        let mut term2 = ratatui::Terminal::new(backend2).unwrap();
        term2.draw(|f| crate::ui::render(f, &app)).unwrap();
        // all tabs render
        let mut app2 = App::new(root.clone());
        for tab in Tab::all() {
            app2.tab = tab;
            term.draw(|f| crate::ui::render(f, &app2)).unwrap();
        }
        let _ = std::fs::remove_dir_all(&root);
    }
}
#[cfg(test)]
mod regression_tests {
    use super::*;
    fn buffer_text(term: &ratatui::Terminal<ratatui::backend::TestBackend>) -> String {
        let buf = term.backend().buffer().clone();
        let mut out = String::new();
        for y in 0..buf.area.height {
            for x in 0..buf.area.width {
                out.push_str(buf[(x, y)].symbol());
            }
            out.push('\n');
        }
        out
    }
    #[test]
    fn tab_titles_and_current_section_visible() {
        let root = std::env::temp_dir().join(format!("obscura-tabs-{}", uuid::Uuid::new_v4()));
        let mut app = App::new(root.clone());
        app.tab = Tab::Cdp;
        let backend = ratatui::backend::TestBackend::new(120, 30);
        let mut term = ratatui::Terminal::new(backend).unwrap();
        term.draw(|f| crate::ui::render(f, &app)).unwrap();
        let text = buffer_text(&term);
        // Old bug: tabs occupied a 2-row bordered block with 0 inner rows, so
        // no title ever rendered.
        assert!(text.contains("1:Dashboard"), "tab titles missing:\n{text}");
        assert!(text.contains("3:CDP"), "CDP tab title missing:\n{text}");
        assert!(
            text.contains("SECTION: CDP"),
            "current-section banner missing:\n{text}"
        );
        // Selected tab cell must carry the highlight background.
        let buf = term.backend().buffer().clone();
        let mut found_highlight = false;
        for y in 0..buf.area.height {
            for x in 0..buf.area.width {
                let cell = &buf[(x, y)];
                if cell.symbol() == "▶" && cell.bg == ratatui::style::Color::Cyan {
                    found_highlight = true;
                }
            }
        }
        assert!(found_highlight, "selected tab highlight (cyan ▶) not found");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn tasks_detail_shows_dump_file_content() {
        let root = std::env::temp_dir().join(format!("obscura-dumpui-{}", uuid::Uuid::new_v4()));
        let mut app = App::new(root.clone());
        // Write a real dump file and register an Exited fetch instance for it.
        let dump_path = crate::config::artifacts_dir(&root)
            .join("dumps")
            .join("probe_example.txt");
        std::fs::write(&dump_path, "DUMP_CONTENT_MARKER hello from dump").unwrap();
        let inst = crate::instance::Instance {
            id: uuid::Uuid::new_v4(),
            params: crate::cli::TaskParams::Fetch(FetchParams::default()),
            argv: vec!["fetch".into(), "https://example.com".into()],
            state: crate::instance::InstanceState::Exited,
            pid: None,
            started_at: "now".into(),
            finished_at: Some("now".into()),
            exit_code: Some(0),
            exit_note: None,
            artifact_paths: vec![dump_path.clone()],
            stdout_log: root.join("a.log"),
            stderr_log: root.join("b.log"),
        };
        app.manager.instances.push(inst);
        app.selected = 0;
        app.tab = Tab::Tasks;
        let backend = ratatui::backend::TestBackend::new(140, 40);
        let mut term = ratatui::Terminal::new(backend).unwrap();
        term.draw(|f| crate::ui::render(f, &app)).unwrap();
        let text = buffer_text(&term);
        assert!(
            text.contains("DUMP_CONTENT_MARKER"),
            "dump preview missing from Tasks detail:\n{text}"
        );
        let _ = std::fs::remove_dir_all(&root);
    }
}
