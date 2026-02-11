use std::time::Duration;

use crossterm::event::{self, Event as CEvent, KeyEvent};
use tokio::sync::mpsc;

#[derive(Debug)]
pub enum InputEvent {
    Key(KeyEvent),
    Paste(String),
}

pub fn spawn_input_reader(tx: mpsc::UnboundedSender<InputEvent>) {
    tokio::task::spawn_blocking(move || loop {
        if event::poll(Duration::from_millis(50)).unwrap_or(false) {
            match event::read() {
                Ok(CEvent::Key(key)) => {
                    let _ = tx.send(InputEvent::Key(key));
                }
                Ok(CEvent::Paste(text)) => {
                    let _ = tx.send(InputEvent::Paste(text));
                }
                _ => {}
            }
        }
    });
}
