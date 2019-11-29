use std::rc::Rc;

use crate::proto::{Command, Event};
use futures::channel::mpsc;
use wasm_bindgen_futures::spawn_local;

use crate::resolve_after;
use futures::stream::LocalBoxStream;

#[mockall::automock]
pub trait RpcClient {
    fn send(&self, cmd: Command);
    fn on_message(&mut self) -> LocalBoxStream<'static, Event>;
}

pub struct WebSocket {
    on_message: Option<mpsc::UnboundedSender<Event>>,
}

impl WebSocket {
    pub fn new() -> Self {
        Self { on_message: None }
    }
}

impl RpcClient for WebSocket {
    fn send(&self, cmd: Command) {
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
            on_message
                .as_ref()
                .map(move |f| f.unbounded_send(event_to_send).unwrap());
        });
    }

    fn on_message(&mut self) -> LocalBoxStream<'static, Event> {
        let (tx, rx) = mpsc::unbounded();
        self.on_message = Some(tx);
        Box::pin(rx)
    }
}
