use std::time::Duration;

use crossterm::event::{self, Event, KeyEvent, KeyEventKind, poll};
use tokio::{sync::mpsc, task::spawn_blocking};

pub enum AppEvent {
    Key(KeyEvent),
    Resize(u16, u16),
    Tick,
}

pub struct EventHandler {
    rx: mpsc::UnboundedReceiver<AppEvent>,
    _task: tokio::task::JoinHandle<()>,
}

impl EventHandler {
    pub fn new(tick_rate: Duration) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        let _task = {
            spawn_blocking(move || {
                loop {
                    let ok = match poll(tick_rate) {
                        Ok(true) => match event::read() {
                            Ok(Event::Key(key)) if key.kind == KeyEventKind::Press => {
                                tx.send(AppEvent::Key(key)).is_ok()
                            }
                            Ok(Event::Resize(w, h)) => {
                                tx.send(AppEvent::Resize(w, h)).is_ok()
                            }
                            Ok(_) => true, // ignore mouse, focus, paste events
                            Err(_) => false,
                        },
                        Ok(false) => tx.send(AppEvent::Tick).is_ok(),
                        Err(_) => false,
                    };
                    if !ok {
                        break;
                    }
                }
            })
        };
        Self { rx, _task }
    }

    pub async fn next(&mut self) -> Option<AppEvent> {
        self.rx.recv().await
    }
}
