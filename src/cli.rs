use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskKind {
    Fetch,
    Serve,
    Scrape,
    Mcp,
}
impl TaskKind {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Fetch => "fetch",
            Self::Serve => "serve",
            Self::Scrape => "scrape",
            Self::Mcp => "mcp",
        }
    }
    #[allow(dead_code)]
    pub fn all() -> [Self; 4] {
        [Self::Fetch, Self::Serve, Self::Scrape, Self::Mcp]
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GlobalOpts {
    pub verbose: bool,
    pub proxy: String,
    pub stealth: bool,
    pub obey_robots: bool,
    pub user_agent: String,
    pub storage_dir: String,
    pub allow_private_network: bool,
    pub v8_flags: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FetchParams {
    pub url: String,
    pub dump: String,
    pub file: String,
    pub concurrency: String,
    pub selector: String,
    pub wait: String,
    pub timeout: String,
    pub wait_until: String,
    pub eval: String,
    pub output: String,
    pub screenshot: String,
    pub quiet: bool,
    pub global: GlobalOpts,
}
impl Default for FetchParams {
    fn default() -> Self {
        Self {
            url: "https://example.com".into(),
            dump: "html".into(),
            file: String::new(),
            concurrency: "1".into(),
            selector: String::new(),
            wait: String::new(),
            timeout: "30".into(),
            wait_until: "load".into(),
            eval: String::new(),
            output: String::new(),
            screenshot: String::new(),
            quiet: false,
            global: GlobalOpts::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServeParams {
    pub port: String,
    pub host: String,
    pub workers: String,
    pub max_connections: String,
    pub ready_file: String,
    pub font_dirs: Vec<String>,
    pub allow_file_access: bool,
    pub quiet: bool,
    pub global: GlobalOpts,
}
impl Default for ServeParams {
    fn default() -> Self {
        Self {
            port: "9222".into(),
            host: "127.0.0.1".into(),
            workers: "1".into(),
            max_connections: "128".into(),
            ready_file: String::new(),
            font_dirs: Vec::new(),
            allow_file_access: false,
            quiet: false,
            global: GlobalOpts::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScrapeParams {
    pub urls: Vec<String>,
    pub eval: String,
    pub concurrency: String,
    pub format: String,
    pub timeout: String,
    pub quiet: bool,
    pub use_stdin: bool,
    pub global: GlobalOpts,
}
impl Default for ScrapeParams {
    fn default() -> Self {
        Self {
            urls: vec!["https://example.com".into()],
            eval: "document.title".into(),
            concurrency: "10".into(),
            format: "json".into(),
            timeout: "60".into(),
            quiet: false,
            use_stdin: false,
            global: GlobalOpts::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpParams {
    pub http: bool,
    pub host: String,
    pub port: String,
    pub global: GlobalOpts,
}
impl Default for McpParams {
    fn default() -> Self {
        Self {
            http: false,
            host: "127.0.0.1".into(),
            port: "3000".into(),
            global: GlobalOpts::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TaskParams {
    Fetch(FetchParams),
    Serve(ServeParams),
    Scrape(ScrapeParams),
    Mcp(McpParams),
}
impl TaskParams {
    pub fn kind(&self) -> TaskKind {
        match self {
            Self::Fetch(_) => TaskKind::Fetch,
            Self::Serve(_) => TaskKind::Serve,
            Self::Scrape(_) => TaskKind::Scrape,
            Self::Mcp(_) => TaskKind::Mcp,
        }
    }
    pub fn target_label(&self) -> String {
        match self {
            Self::Fetch(p) => {
                if !p.file.is_empty() {
                    format!("file:{}", p.file)
                } else if p.url.is_empty() {
                    "(no url)".into()
                } else {
                    p.url.clone()
                }
            }
            Self::Serve(p) => format!("{}:{}", p.host, p.port),
            Self::Scrape(p) => {
                if p.use_stdin {
                    "stdin".into()
                } else {
                    format!("{} urls", p.urls.len())
                }
            }
            Self::Mcp(p) => {
                if p.http {
                    format!("http {}:{}", p.host, p.port)
                } else {
                    "stdio".into()
                }
            }
        }
    }
    pub fn port(&self) -> Option<u16> {
        match self {
            Self::Serve(p) => p.port.parse().ok(),
            Self::Mcp(p) if p.http => p.port.parse().ok(),
            _ => None,
        }
    }
}

fn push_global(args: &mut Vec<String>, g: &GlobalOpts, include_port: bool, port: Option<&str>) {
    if g.verbose {
        args.push("--verbose".into());
    }
    if include_port
        && let Some(p) = port
        && !p.is_empty()
    {
        args.push("--port".into());
        args.push(p.to_string());
    }
    if !g.proxy.is_empty() {
        args.push("--proxy".into());
        args.push(g.proxy.clone());
    }
    if g.stealth {
        args.push("--stealth".into());
    }
    if g.obey_robots {
        args.push("--obey-robots".into());
    }
    if !g.user_agent.is_empty() {
        args.push("--user-agent".into());
        args.push(g.user_agent.clone());
    }
    if !g.storage_dir.is_empty() {
        args.push("--storage-dir".into());
        args.push(g.storage_dir.clone());
    }
    if g.allow_private_network {
        args.push("--allow-private-network".into());
    }
    if !g.v8_flags.is_empty() {
        args.push("--v8-flags".into());
        args.push(g.v8_flags.clone());
    }
}

pub fn build_argv(params: &TaskParams) -> Vec<String> {
    let mut a: Vec<String> = Vec::new();
    match params {
        TaskParams::Fetch(p) => {
            // global flags that fetch accepts as global (before subcommand in real CLI also works after; we emit after)
            a.push("fetch".into());
            if !p.file.is_empty() {
                a.push("--file".into());
                a.push(p.file.clone());
                if !p.concurrency.is_empty() && p.concurrency != "1" {
                    a.push("--concurrency".into());
                    a.push(p.concurrency.clone());
                }
            } else if !p.url.is_empty() {
                a.push(p.url.clone());
            }
            if !p.dump.is_empty() && p.dump != "html" {
                a.push("--dump".into());
                a.push(p.dump.clone());
            } else if p.dump == "html" {
                // keep explicit for clarity in logs
                a.push("--dump".into());
                a.push("html".into());
            }
            if !p.selector.is_empty() {
                a.push("--selector".into());
                a.push(p.selector.clone());
            }
            if !p.wait.is_empty() {
                a.push("--wait".into());
                a.push(p.wait.clone());
            }
            if !p.timeout.is_empty() {
                a.push("--timeout".into());
                a.push(p.timeout.clone());
            }
            if !p.wait_until.is_empty() && p.wait_until != "load" {
                a.push("--wait-until".into());
                a.push(p.wait_until.clone());
            }
            if !p.eval.is_empty() {
                a.push("--eval".into());
                a.push(p.eval.clone());
            }
            if !p.output.is_empty() {
                a.push("--output".into());
                a.push(p.output.clone());
            }
            if !p.screenshot.is_empty() {
                a.push("--screenshot".into());
                a.push(p.screenshot.clone());
            }
            if p.quiet {
                a.push("--quiet".into());
            }
            // per-subcommand duplicates accepted by obscura (proxy/stealth/etc are global)
            push_global(&mut a, &p.global, false, None);
        }
        TaskParams::Serve(p) => {
            a.push("serve".into());
            if !p.port.is_empty() {
                a.push("--port".into());
                a.push(p.port.clone());
            }
            if !p.host.is_empty() {
                a.push("--host".into());
                a.push(p.host.clone());
            }
            if !p.workers.is_empty() && p.workers != "1" {
                a.push("--workers".into());
                a.push(p.workers.clone());
            }
            if !p.max_connections.is_empty() && p.max_connections != "128" {
                a.push("--max-connections".into());
                a.push(p.max_connections.clone());
            }
            if !p.ready_file.is_empty() {
                a.push("--ready-file".into());
                a.push(p.ready_file.clone());
            }
            for d in &p.font_dirs {
                if !d.is_empty() {
                    a.push("--font-dir".into());
                    a.push(d.clone());
                }
            }
            if p.allow_file_access {
                a.push("--allow-file-access".into());
            }
            if p.quiet {
                a.push("--quiet".into());
            }
            push_global(&mut a, &p.global, false, None);
        }
        TaskParams::Scrape(p) => {
            a.push("scrape".into());
            if p.use_stdin {
                a.push("-".into());
            } else {
                for u in &p.urls {
                    if !u.is_empty() {
                        a.push(u.clone());
                    }
                }
            }
            if !p.eval.is_empty() {
                a.push("--eval".into());
                a.push(p.eval.clone());
            }
            if !p.concurrency.is_empty() && p.concurrency != "10" {
                a.push("--concurrency".into());
                a.push(p.concurrency.clone());
            }
            if !p.format.is_empty() && p.format != "json" {
                a.push("--format".into());
                a.push(p.format.clone());
            }
            if !p.timeout.is_empty() && p.timeout != "60" {
                a.push("--timeout".into());
                a.push(p.timeout.clone());
            }
            if p.quiet {
                a.push("--quiet".into());
            }
            push_global(&mut a, &p.global, false, None);
        }
        TaskParams::Mcp(p) => {
            a.push("mcp".into());
            if p.http {
                a.push("--http".into());
                if !p.host.is_empty() {
                    a.push("--host".into());
                    a.push(p.host.clone());
                }
                if !p.port.is_empty() {
                    a.push("--port".into());
                    a.push(p.port.clone());
                }
            }
            push_global(&mut a, &p.global, false, None);
        }
    }
    a
}

/// Resolve a user-typed `--output` value: absolute paths are kept, relative
/// names are placed inside `artifacts/dumps/` (not the manager's CWD, where
/// the Artifacts tab would never find them).
pub fn resolve_dump_output(root: &std::path::Path, output: &str) -> PathBuf {
    let p = PathBuf::from(output);
    if p.is_absolute() {
        p
    } else {
        crate::config::artifacts_dir(root).join("dumps").join(p)
    }
}

pub fn default_output_path(root: &std::path::Path, params: &TaskParams) -> Option<PathBuf> {
    match params {
        TaskParams::Fetch(p) => {
            if !p.output.is_empty() {
                return Some(PathBuf::from(&p.output));
            }
            let ext = match p.dump.as_str() {
                "markdown" => "md",
                "links" | "text" => "txt",
                "assets" | "cookies" => "json",
                "original" => "bin",
                _ => "html",
            };
            // Unique per spawn: second-resolution timestamps alone made
            // repeated fetches of the same host overwrite the previous dump.
            let unique = &uuid::Uuid::new_v4().to_string()[..6];
            let name = format!(
                "{}_{}_{}.{}",
                crate::config::timestamp_slug(),
                crate::config::host_slug(&p.url),
                unique,
                ext
            );
            Some(crate::config::artifacts_dir(root).join("dumps").join(name))
        }
        TaskParams::Scrape(p) => {
            let ext = if p.format == "json" { "json" } else { "txt" };
            let name = format!("{}_scrape.{}", crate::config::timestamp_slug(), ext);
            Some(
                crate::config::artifacts_dir(root)
                    .join("scrapes")
                    .join(name),
            )
        }
        _ => None,
    }
}

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)]
mod tests {
    use super::*;
    #[test]
    fn fetch_argv_includes_dump_eval_output() {
        let mut p = FetchParams::default();
        p.url = "https://example.com".into();
        p.eval = "document.title".into();
        p.output = "/tmp/out.html".into();
        let argv = build_argv(&TaskParams::Fetch(p));
        assert_eq!(argv[0], "fetch");
        assert!(argv.contains(&"--eval".to_string()));
        assert!(argv.contains(&"document.title".to_string()));
        assert!(argv.contains(&"--output".to_string()));
    }
    #[test]
    fn fetch_batch_file_mode() {
        let mut p = FetchParams::default();
        p.file = "urls.txt".into();
        p.concurrency = "5".into();
        let argv = build_argv(&TaskParams::Fetch(p));
        assert!(argv.contains(&"--file".to_string()));
        assert!(argv.contains(&"--concurrency".to_string()));
    }
    #[test]
    fn serve_argv_port_host_workers() {
        let mut p = ServeParams::default();
        p.port = "9333".into();
        p.workers = "4".into();
        p.max_connections = "10".into();
        let argv = build_argv(&TaskParams::Serve(p));
        assert_eq!(argv[0], "serve");
        assert!(argv.windows(2).any(|w| w == ["--port", "9333"]));
        assert!(argv.windows(2).any(|w| w == ["--workers", "4"]));
        assert!(argv.windows(2).any(|w| w == ["--max-connections", "10"]));
    }
    #[test]
    fn scrape_stdin_and_eval() {
        let mut p = ScrapeParams::default();
        p.use_stdin = true;
        p.eval = "document.title".into();
        let argv = build_argv(&TaskParams::Scrape(p));
        assert!(argv.contains(&"-".to_string()));
        assert!(argv.contains(&"--eval".to_string()));
    }
    #[test]
    fn mcp_http_argv() {
        let mut p = McpParams::default();
        p.http = true;
        p.port = "8080".into();
        let argv = build_argv(&TaskParams::Mcp(p));
        assert!(argv.contains(&"--http".to_string()));
        assert!(argv.windows(2).any(|w| w == ["--port", "8080"]));
    }
    #[test]
    fn global_flags_propagate() {
        let mut p = FetchParams::default();
        p.global.stealth = true;
        p.global.proxy = "socks5://127.0.0.1:1080".into();
        p.global.allow_private_network = true;
        let argv = build_argv(&TaskParams::Fetch(p));
        assert!(argv.contains(&"--stealth".to_string()));
        assert!(argv.contains(&"--allow-private-network".to_string()));
        assert!(argv.contains(&"socks5://127.0.0.1:1080".to_string()));
    }
}

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)]
mod dump_regression_tests {
    use super::*;
    #[test]
    fn default_dump_paths_are_unique_and_in_dumps_dir() {
        let root = std::path::Path::new("/tmp/obscura-root-probe");
        let p = TaskParams::Fetch(FetchParams::default());
        let a = default_output_path(root, &p).unwrap();
        let b = default_output_path(root, &p).unwrap();
        assert_ne!(a, b, "repeated dumps must not overwrite each other");
        assert!(a.to_string_lossy().contains("artifacts/dumps"));
        assert!(a.extension().is_some_and(|e| e == "html"));
    }
    #[test]
    fn dump_extension_matches_format() {
        let root = std::path::Path::new("/tmp/obscura-root-probe");
        for (dump, ext) in [
            ("text", "txt"),
            ("markdown", "md"),
            ("links", "txt"),
            ("assets", "json"),
            ("cookies", "json"),
        ] {
            let mut f = FetchParams::default();
            f.dump = dump.into();
            let path = default_output_path(root, &TaskParams::Fetch(f)).unwrap();
            assert_eq!(
                path.extension().and_then(|e| e.to_str()),
                Some(ext),
                "dump={dump}"
            );
        }
    }
    #[test]
    fn relative_output_resolves_into_artifacts_dumps() {
        let root = std::path::Path::new("/tmp/obscura-root-probe");
        let rel = resolve_dump_output(root, "my-page.txt");
        assert_eq!(rel, root.join("artifacts/dumps/my-page.txt"));
        let abs = resolve_dump_output(root, "/elsewhere/x.html");
        assert_eq!(abs, std::path::PathBuf::from("/elsewhere/x.html"));
    }
}
