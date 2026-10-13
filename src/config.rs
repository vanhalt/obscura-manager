use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub const ARTIFACT_SUBDIRS: &[&str] =
    &["screenshots", "dumps", "scrapes", "pdfs", "exports", "logs"];

pub fn resolve_root(cli_root: Option<&Path>) -> PathBuf {
    if let Some(p) = cli_root {
        return p.to_path_buf();
    }
    if let Ok(v) = std::env::var("OBSCURA_ROOT")
        && !v.trim().is_empty()
    {
        return PathBuf::from(v);
    }
    if let Ok(v) = std::env::var("XDG_DATA_HOME")
        && !v.trim().is_empty()
    {
        return PathBuf::from(v).join("obscura-manager");
    }
    if let Ok(v) = std::env::var("HOME") {
        return PathBuf::from(v).join(".local/share/obscura-manager");
    }
    PathBuf::from(".obscura-manager")
}

pub fn artifacts_dir(root: &Path) -> PathBuf {
    root.join("artifacts")
}

pub fn ensure_layout(root: &Path) -> anyhow::Result<()> {
    std::fs::create_dir_all(root)?;
    for sub in ARTIFACT_SUBDIRS {
        std::fs::create_dir_all(artifacts_dir(root).join(sub))?;
    }
    Ok(())
}

pub fn resolve_binary(root: &Path) -> Option<PathBuf> {
    if let Ok(v) = std::env::var("OBSCURA_BIN")
        && !v.trim().is_empty()
    {
        let p = PathBuf::from(&v);
        if p.exists() {
            return Some(p);
        }
    }
    // PATH lookup
    if let Ok(paths) = std::env::var("PATH") {
        for dir in std::env::split_paths(&paths) {
            let cand = dir.join("obscura");
            if cand.exists() {
                return Some(cand);
            }
        }
    }
    let cand = root.join("bin/obscura");
    if cand.exists() {
        return Some(cand);
    }
    None
}

pub fn obscura_version(bin: Option<&Path>) -> String {
    let Some(bin) = bin else {
        return "not found (set OBSCURA_BIN or install obscura)".to_string();
    };
    match std::process::Command::new(bin).arg("--version").output() {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).trim().to_string(),
        Ok(o) => format!("error: {}", String::from_utf8_lossy(&o.stderr).trim()),
        Err(e) => format!("not found: {e}"),
    }
}

pub const KNOWN_ENV_VARS: &[&str] = &[
    "OBSCURA_ALLOW_PRIVATE_NETWORK",
    "OBSCURA_NAV_TIMEOUT_MS",
    "OBSCURA_NAV_CHAIN_LIMIT",
    "OBSCURA_SCRIPT_DEADLINE_MS",
    "OBSCURA_MODULE_BUDGET_MS",
    "OBSCURA_CDP_COMMAND_TIMEOUT_MS",
    "OBSCURA_CDP_TOKEN",
    "OBSCURA_FETCH_TIMEOUT_MS",
    "OBSCURA_PROXY",
    "OBSCURA_BLOCK_TRACKERS",
    "OBSCURA_TIMEZONE",
    "OBSCURA_GEOLOCATION",
    "OBSCURA_PROFILE",
    "OBSCURA_ROTATE_PROFILE",
    "OBSCURA_MCP_ALLOWED_ORIGINS",
    "OBSCURA_MCP_TOKEN",
    "RUST_LOG",
];

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct ManagerConfig {
    pub env_overrides: BTreeMap<String, String>,
    pub default_proxy: String,
    pub default_user_agent: String,
    pub stealth: bool,
}

impl ManagerConfig {
    pub fn load(root: &Path) -> Self {
        let p = root.join("config.json");
        if let Ok(data) = std::fs::read_to_string(&p)
            && let Ok(c) = serde_json::from_str(&data)
        {
            return c;
        }
        let mut c = Self::default();
        for k in KNOWN_ENV_VARS {
            if let Ok(v) = std::env::var(k) {
                c.env_overrides.insert((*k).to_string(), v);
            }
        }
        c
    }
    pub fn save(&self, root: &Path) -> anyhow::Result<()> {
        let p = root.join("config.json");
        std::fs::write(p, serde_json::to_string_pretty(self)?)?;
        // also write human-readable config.toml mirror
        let mut toml = String::from("# obscura-manager config\n");
        for (k, v) in &self.env_overrides {
            toml.push_str(&format!("{k} = \"{}\"\n", v.replace('"', "\\\"")));
        }
        std::fs::write(root.join("config.toml"), toml)?;
        Ok(())
    }
}

pub fn timestamp_slug() -> String {
    chrono::Local::now().format("%Y-%m-%dT%H-%M-%S").to_string()
}

pub fn host_slug(url: &str) -> String {
    let s = url
        .trim_start_matches("https://")
        .trim_start_matches("http://");
    let host = s.split('/').next().unwrap_or("page");
    host.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '.' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn resolve_root_prefers_env() {
        // cli arg wins
        let p = resolve_root(Some(Path::new("/tmp/custom-root")));
        assert_eq!(p, PathBuf::from("/tmp/custom-root"));
    }
    #[test]
    fn ensure_layout_creates_artifacts() {
        let dir = std::env::temp_dir().join(format!("obscura-test-{}", uuid::Uuid::new_v4()));
        ensure_layout(&dir).unwrap();
        for sub in ARTIFACT_SUBDIRS {
            assert!(dir.join("artifacts").join(sub).is_dir(), "missing {sub}");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
    #[test]
    fn host_slug_sanitizes() {
        assert_eq!(host_slug("https://example.com/path?q=1"), "example.com");
    }
}
