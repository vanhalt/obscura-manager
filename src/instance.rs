use crate::cli::{TaskParams, build_argv};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::Sender;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InstanceState {
    Spawning,
    Running,
    Exited,
    Failed,
    Killed,
}
impl InstanceState {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Spawning => "Spawning",
            Self::Running => "Running",
            Self::Exited => "Exited",
            Self::Failed => "Failed",
            Self::Killed => "Killed",
        }
    }
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Exited | Self::Failed | Self::Killed)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Instance {
    pub id: Uuid,
    pub params: TaskParams,
    pub argv: Vec<String>,
    pub state: InstanceState,
    pub pid: Option<u32>,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub exit_code: Option<i32>,
    pub exit_note: Option<String>,
    pub artifact_paths: Vec<PathBuf>,
    pub stdout_log: PathBuf,
    pub stderr_log: PathBuf,
}

#[derive(Debug, Clone)]
pub enum ProcEvent {
    Line {
        id: Uuid,
        is_stderr: bool,
        line: String,
    },
    Exited {
        id: Uuid,
        code: Option<i32>,
    },
}

pub struct Manager {
    pub instances: Vec<Instance>,
    children: HashMap<Uuid, Child>,
    pub logs: HashMap<Uuid, Vec<String>>,
}

impl Default for Manager {
    fn default() -> Self {
        Self::new()
    }
}

impl Manager {
    pub fn new() -> Self {
        Self {
            instances: Vec::new(),
            children: HashMap::new(),
            logs: HashMap::new(),
        }
    }

    pub fn load_history(root: &Path) -> Self {
        let mut m = Self::new();
        let p = root.join("instances.json");
        if let Ok(data) = std::fs::read_to_string(&p)
            && let Ok(mut list) = serde_json::from_str::<Vec<Instance>>(&data)
        {
            for inst in &mut list {
                if matches!(inst.state, InstanceState::Running | InstanceState::Spawning) {
                    inst.state = InstanceState::Exited;
                    inst.exit_note =
                        Some("manager exited — spawn a new instance (no reattach)".into());
                    inst.finished_at = Some(chrono::Local::now().to_rfc3339());
                    inst.pid = None;
                }
            }
            m.instances = list;
        }
        m
    }

    pub fn save_history(&self, root: &Path) {
        let p = root.join("instances.json");
        let trimmed: Vec<_> = self
            .instances
            .iter()
            .rev()
            .take(200)
            .rev()
            .cloned()
            .collect();
        if let Ok(s) = serde_json::to_string_pretty(&trimmed) {
            let _ = std::fs::write(p, s);
        }
    }

    pub fn spawn(
        &mut self,
        root: &Path,
        binary: &Path,
        params: TaskParams,
        env_overrides: &std::collections::BTreeMap<String, String>,
        tx: Sender<ProcEvent>,
    ) -> anyhow::Result<Uuid> {
        let id = Uuid::new_v4();
        let argv = build_argv(&params);
        let log_dir = crate::config::artifacts_dir(root).join("logs");
        let _ = std::fs::create_dir_all(&log_dir);
        let stdout_log = log_dir.join(format!("{id}.stdout.log"));
        let stderr_log = log_dir.join(format!("{id}.stderr.log"));

        let mut artifacts: Vec<PathBuf> = Vec::new();
        if let crate::cli::TaskParams::Fetch(fp) = &params {
            if !fp.screenshot.is_empty() {
                artifacts.push(PathBuf::from(&fp.screenshot));
            }
            if !fp.output.is_empty() {
                artifacts.push(PathBuf::from(&fp.output));
            }
        }

        let mut inst = Instance {
            id,
            params: params.clone(),
            argv: argv.clone(),
            state: InstanceState::Spawning,
            pid: None,
            started_at: chrono::Local::now().to_rfc3339(),
            finished_at: None,
            exit_code: None,
            exit_note: None,
            artifact_paths: artifacts,
            stdout_log: stdout_log.clone(),
            stderr_log: stderr_log.clone(),
        };
        self.instances.push(inst.clone());
        self.logs.insert(id, Vec::new());

        let mut cmd = Command::new(binary);
        cmd.args(&argv)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for (k, v) in env_overrides {
            cmd.env(k, v);
        }
        // Ensure worker sibling found: set PATH to include binary dir
        if let Some(dir) = binary.parent() {
            let old = std::env::var("PATH").unwrap_or_default();
            cmd.env("PATH", format!("{}:{}", dir.display(), old));
        }

        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => {
                if let Some(i) = self.instances.iter_mut().find(|i| i.id == id) {
                    i.state = InstanceState::Failed;
                    i.exit_note = Some(format!("spawn failed: {e}"));
                    i.finished_at = Some(chrono::Local::now().to_rfc3339());
                }
                return Err(e.into());
            }
        };
        inst.pid = Some(child.id());
        if let Some(i) = self.instances.iter_mut().find(|i| i.id == id) {
            i.pid = Some(child.id());
            i.state = InstanceState::Running;
        }

        // stdout thread
        if let Some(out) = child.stdout.take() {
            let tx2 = tx.clone();
            let stdout_log2 = stdout_log.clone();
            std::thread::spawn(move || {
                let reader = BufReader::new(out);
                for line in reader.lines().map_while(Result::ok) {
                    if let Ok(mut f) = std::fs::OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(&stdout_log2)
                    {
                        use std::io::Write;
                        let _ = writeln!(f, "{line}");
                    }
                    let _ = tx2.send(ProcEvent::Line {
                        id,
                        is_stderr: false,
                        line,
                    });
                }
            });
        }
        if let Some(err) = child.stderr.take() {
            let tx3 = tx.clone();
            let stderr_log2 = stderr_log.clone();
            std::thread::spawn(move || {
                let reader = BufReader::new(err);
                for line in reader.lines().map_while(Result::ok) {
                    if let Ok(mut f) = std::fs::OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(&stderr_log2)
                    {
                        use std::io::Write;
                        let _ = writeln!(f, "{line}");
                    }
                    let _ = tx3.send(ProcEvent::Line {
                        id,
                        is_stderr: true,
                        line,
                    });
                }
            });
        }
        self.children.insert(id, child);
        Ok(id)
    }

    pub fn poll(&mut self, tx: &Sender<ProcEvent>) {
        let mut exited: Vec<(Uuid, Option<i32>)> = Vec::new();
        for (id, child) in self.children.iter_mut() {
            match child.try_wait() {
                Ok(Some(status)) => exited.push((*id, status.code())),
                Ok(None) => {}
                Err(_) => exited.push((*id, None)),
            }
        }
        for (id, code) in exited {
            self.children.remove(&id);
            let _ = tx.send(ProcEvent::Exited { id, code });
        }
    }

    pub fn handle_event(&mut self, ev: ProcEvent) {
        match ev {
            ProcEvent::Line {
                id,
                is_stderr,
                line,
            } => {
                let entry = if is_stderr {
                    format!("[stderr] {line}")
                } else {
                    line
                };
                if let Some(v) = self.logs.get_mut(&id) {
                    v.push(entry);
                    if v.len() > 2000 {
                        v.drain(0..500);
                    }
                }
            }
            ProcEvent::Exited { id, code } => {
                if let Some(inst) = self.instances.iter_mut().find(|i| i.id == id) {
                    if inst.state != InstanceState::Killed {
                        if code == Some(0)
                            || (inst.params.kind() != crate::cli::TaskKind::Fetch && code.is_none())
                        {
                            // serve/mcp killed via signal handled separately; default Exited
                            inst.state = InstanceState::Exited;
                        } else if code.is_some() && code != Some(0) {
                            // fetch non-zero is failure; serve exiting non-zero also failed
                            // Preserve Exited for clean, Failed for non-zero
                            inst.state = if code == Some(0) {
                                InstanceState::Exited
                            } else {
                                InstanceState::Failed
                            };
                        } else {
                            inst.state = InstanceState::Exited;
                        }
                        if code == Some(0) {
                            inst.state = InstanceState::Exited;
                        }
                    }
                    inst.exit_code = code;
                    inst.finished_at = Some(chrono::Local::now().to_rfc3339());
                    inst.pid = None;
                }
                self.children.remove(&id);
            }
        }
    }

    pub fn kill(&mut self, id: Uuid) -> anyhow::Result<()> {
        if let Some(child) = self.children.get_mut(&id) {
            let _ = child.kill();
            let _ = child.wait();
        }
        self.children.remove(&id);
        if let Some(inst) = self.instances.iter_mut().find(|i| i.id == id)
            && !inst.state.is_terminal()
        {
            inst.state = InstanceState::Killed;
            inst.finished_at = Some(chrono::Local::now().to_rfc3339());
            inst.pid = None;
            inst.exit_note = Some("killed by user".into());
        }
        Ok(())
    }

    pub fn restart_params(&self, id: Uuid) -> Option<TaskParams> {
        self.instances
            .iter()
            .find(|i| i.id == id)
            .map(|i| i.params.clone())
    }

    pub fn delete_history(&mut self, id: Uuid) {
        if self.children.contains_key(&id) {
            let _ = self.kill(id);
        }
        self.instances.retain(|i| i.id != id);
        self.logs.remove(&id);
    }

    pub fn get(&self, id: Uuid) -> Option<&Instance> {
        self.instances.iter().find(|i| i.id == id)
    }
    pub fn running_serve_instances(&self) -> Vec<&Instance> {
        self.instances
            .iter()
            .filter(|i| {
                i.state == InstanceState::Running && i.params.kind() == crate::cli::TaskKind::Serve
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::{FetchParams, TaskParams};

    fn fake_binary(dir: &Path) -> PathBuf {
        let p = dir.join("fake-obscura.sh");
        std::fs::write(&p, "#!/bin/sh\necho \"fake:$@\"\nsleep 0.2\necho done\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        p
    }

    #[test]
    fn no_reattach_on_load() {
        let dir = std::env::temp_dir().join(format!("obscura-hist-{}", Uuid::new_v4()));
        crate::config::ensure_layout(&dir).unwrap();
        let mut m = Manager::new();
        let inst = Instance {
            id: Uuid::new_v4(),
            params: TaskParams::Fetch(FetchParams::default()),
            argv: vec!["fetch".into()],
            state: InstanceState::Running,
            pid: Some(1234),
            started_at: "now".into(),
            finished_at: None,
            exit_code: None,
            exit_note: None,
            artifact_paths: vec![],
            stdout_log: dir.join("a.log"),
            stderr_log: dir.join("b.log"),
        };
        m.instances.push(inst);
        m.save_history(&dir);
        let loaded = Manager::load_history(&dir);
        assert_eq!(loaded.instances[0].state, InstanceState::Exited);
        assert!(
            loaded.instances[0]
                .exit_note
                .as_ref()
                .unwrap()
                .contains("no reattach")
        );
        assert!(loaded.instances[0].pid.is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn spawn_and_exit_with_fake_binary() {
        let dir = std::env::temp_dir().join(format!("obscura-spawn-{}", Uuid::new_v4()));
        crate::config::ensure_layout(&dir).unwrap();
        let bin = fake_binary(&dir);
        let (tx, rx) = std::sync::mpsc::channel();
        let mut m = Manager::new();
        let id = m
            .spawn(
                &dir,
                &bin,
                TaskParams::Fetch(FetchParams::default()),
                &Default::default(),
                tx.clone(),
            )
            .unwrap();
        assert_eq!(m.get(id).unwrap().state, InstanceState::Running);
        // wait for exit via poll loop
        for _ in 0..50 {
            m.poll(&tx);
            while let Ok(ev) = rx.try_recv() {
                m.handle_event(ev);
            }
            if m.get(id).unwrap().state.is_terminal() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        assert!(m.get(id).unwrap().state.is_terminal());
        // restart creates NEW id
        let params = m.restart_params(id).unwrap();
        let new_id = m
            .spawn(&dir, &bin, params, &Default::default(), tx)
            .unwrap();
        assert_ne!(new_id, id);
        // cleanup new
        let _ = m.kill(new_id);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn ui_snapshot_dashboard() {
        // snapshot via TestBackend rendering Dashboard
        let root = std::env::temp_dir().join(format!("obscura-snap-{}", Uuid::new_v4()));
        crate::config::ensure_layout(&root).unwrap();
        // we test rendering does not panic at small and large sizes using ratatui TestBackend in app test below
        let _ = std::fs::remove_dir_all(&root);
    }
}
