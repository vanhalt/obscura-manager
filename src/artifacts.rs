use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Artifact {
    pub path: PathBuf,
    pub kind: String,
    pub size: u64,
    pub modified: String,
}

pub fn scan(root: &Path) -> Vec<Artifact> {
    let base = crate::config::artifacts_dir(root);
    let mut out = Vec::new();
    for sub in crate::config::ARTIFACT_SUBDIRS {
        let dir = base.join(sub);
        if let Ok(rd) = std::fs::read_dir(&dir) {
            for e in rd.flatten() {
                let p = e.path();
                if p.is_file() {
                    let meta = e.metadata().ok();
                    let size = meta.as_ref().map(|m| m.len()).unwrap_or(0);
                    let modified = meta
                        .and_then(|m| m.modified().ok())
                        .map(|t| {
                            let dt: chrono::DateTime<chrono::Local> = t.into();
                            dt.format("%Y-%m-%d %H:%M").to_string()
                        })
                        .unwrap_or_default();
                    out.push(Artifact {
                        path: p,
                        kind: sub.to_string(),
                        size,
                        modified,
                    });
                }
            }
        }
    }
    out.sort_by(|a, b| b.modified.cmp(&a.modified));
    out
}

pub fn preview(path: &Path, max_bytes: usize) -> String {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    let text_exts = ["txt", "csv", "json", "md", "html", "log", "xml"];
    if text_exts.contains(&ext.as_str()) {
        match std::fs::read(path) {
            Ok(bytes) => {
                let slice = &bytes[..bytes.len().min(max_bytes)];
                String::from_utf8_lossy(slice).to_string()
            }
            Err(e) => format!("error reading file: {e}"),
        }
    } else {
        match std::fs::metadata(path) {
            Ok(m) => format!(
                "[binary file: {} bytes — {}]\nPath: {}",
                m.len(),
                ext,
                path.display()
            ),
            Err(e) => format!("error: {e}"),
        }
    }
}

pub fn human_size(n: u64) -> String {
    if n < 1024 {
        format!("{n} B")
    } else if n < 1024 * 1024 {
        format!("{:.1} KB", n as f64 / 1024.0)
    } else {
        format!("{:.1} MB", n as f64 / (1024.0 * 1024.0))
    }
}
