use crossterm::event::{self, Event, KeyEvent};
use std::time::Duration;

pub fn poll_event(timeout: Duration) -> anyhow::Result<Option<KeyEvent>> {
    if event::poll(timeout)?
        && let Event::Key(key) = event::read()?
    {
        return Ok(Some(key));
    }
    Ok(None)
}
