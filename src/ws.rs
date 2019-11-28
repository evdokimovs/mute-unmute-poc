use std::rc::Rc;

use crate::proto::{Command, Event};
use wasm_bindgen_futures::spawn_local;

use crate::resolve_after;

pub struct WebSocket {
    on_message: Option<Rc<dyn Fn(Event)>>,
}

impl WebSocket {
    pub fn new() -> Self {
        Self { on_message: None }
    }

    pub fn send(&self, cmd: Command) {
        web_sys::console::log_1(&"Received command.".into());
        let event_to_send = match cmd {
            Command::MuteRoom { audio, video } => {
                Event::RoomMuted { audio, video }
            }
            Command::UnmuteRoom { audio, video } => {
                Event::RoomUnmuted { audio, video }
            }
        };

        let on_message = self.on_message.clone();
        spawn_local(async move {
            resolve_after(3000).await;
            on_message.as_ref().map(move |f| (f)(event_to_send));
        });
    }

    pub fn on_message<F>(&mut self, on_message: F)
    where
        F: Fn(Event) + 'static,
    {
        self.on_message = Some(Rc::new(on_message));
    }
}
