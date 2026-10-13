use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::{ExecutableCommand, execute};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use std::io::{Stdout, stdout};

pub type Tui = Terminal<CrosstermBackend<Stdout>>;

pub fn init() -> anyhow::Result<Tui> {
    terminal::enable_raw_mode()?;
    let mut out = stdout();
    out.execute(EnterAlternateScreen)?;
    execute!(out, crossterm::event::EnableMouseCapture)?;
    let backend = CrosstermBackend::new(out);
    let mut term = Terminal::new(backend)?;
    term.clear()?;
    Ok(term)
}

pub fn restore(term: &mut Tui) -> anyhow::Result<()> {
    terminal::disable_raw_mode()?;
    execute!(
        term.backend_mut(),
        LeaveAlternateScreen,
        crossterm::event::DisableMouseCapture
    )?;
    term.show_cursor()?;
    Ok(())
}

pub fn install_panic_hook() {
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = terminal::disable_raw_mode();
        let _ = execute!(
            stdout(),
            LeaveAlternateScreen,
            crossterm::event::DisableMouseCapture
        );
        hook(info);
    }));
}
