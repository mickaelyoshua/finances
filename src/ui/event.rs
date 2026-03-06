use std::time::Duration;

use crossterm::event::{self, Event, KeyEvent, KeyEventKind, poll};
use tokio::{sync::mpsc, task::spawn_blocking};

pub enum AppEvent {
    Key(KeyEvent),
    Tick, // periodic tick for animations/refreshes
}

pub struct EventHandler {
    rx: mpsc::UnboundedReceiver<AppEvent>,
    // the background task handle, to abort on exit
    _task: tokio::task::JoinHandle<()>,
}

impl EventHandler {
    pub fn new(tick_rate: Duration) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        let _task = {
            spawn_blocking(move || {
                loop {
                    if poll(tick_rate).expect("failed to poll events") {
                        if let Event::Key(key) = event::read().expect("failed to read event")
                            && key.kind == KeyEventKind::Press
                            && tx.send(AppEvent::Key(key)).is_err()
                        {
                            break;
                        }
                    } else if tx.send(AppEvent::Tick).is_err() {
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
