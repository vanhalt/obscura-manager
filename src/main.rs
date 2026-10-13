mod app;
mod artifacts;
mod cdp;
mod cli;
mod config;
mod event;
mod instance;
mod tui;
mod ui;
mod update;

use app::App;
use std::path::PathBuf;
use std::time::Duration;

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let mut root_arg: Option<PathBuf> = None;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--root" => {
                if i + 1 < args.len() {
                    root_arg = Some(PathBuf::from(&args[i + 1]));
                    i += 1;
                }
            }
            "--help" | "-h" => {
                println!(
                    "obscura-manager — Ratatui manager for Obscura\n\nUsage: obscura-manager [--root <DIR>]\n\nEnv: OBSCURA_ROOT, OBSCURA_BIN\nKeys: q quit, ? help, 1-7 tabs, n new task (see Help tab)"
                );
                return Ok(());
            }
            "--version" | "-V" => {
                println!("obscura-manager {}", env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            other => {
                eprintln!("unknown arg: {other} (use --help)");
            }
        }
        i += 1;
    }
    let root = config::resolve_root(root_arg.as_deref());
    config::ensure_layout(&root)?;

    tui::install_panic_hook();
    let mut terminal = tui::init()?;
    let mut app = App::new(root);
    let res = run(&mut terminal, &mut app);
    // save history on exit
    app.manager.save_history(&app.root);
    let _ = app.config.save(&app.root);
    tui::restore(&mut terminal)?;
    res
}

fn run(terminal: &mut tui::Tui, app: &mut App) -> anyhow::Result<()> {
    loop {
        terminal.draw(|f| ui::render(f, app))?;
        if let Some(key) = event::poll_event(Duration::from_millis(50))? {
            // only handle key press (not release) on crossterm
            if key.kind == crossterm::event::KeyEventKind::Release {
                continue;
            }
            update::handle_key(app, key);
            if app.should_quit {
                break;
            }
        }
        app.on_tick();
    }
    Ok(())
}
