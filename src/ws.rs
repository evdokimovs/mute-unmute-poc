use std::rc::Rc;

use crate::event_listener::EventListener;
use futures::SinkExt;
use mute_unmute_poc_proto::{Command, Event};
use web_sys::WebSocket as SysWebSocket;

pub struct WebSocket {
    socket: Rc<SysWebSocket>,
    on_message: Option<EventListener<SysWebSocket, web_sys::MessageEvent>>,
}

impl WebSocket {
    pub fn new(url: &str) -> Self {
        let socket = Rc::new(SysWebSocket::new(url).unwrap());
        Self {
            socket,
            on_message: None,
        }
    }

    /// Set handler on receive message from server.
    pub fn on_message<F>(&mut self, mut f: F)
    where
        F: (FnMut(Event)) + 'static,
    {
        self.on_message = Some(EventListener::new_mut(
            Rc::clone(&self.socket),
            "message",
            move |msg: web_sys::MessageEvent| {
                let parsed: Event =
                    serde_json::from_str(&msg.data().as_string().unwrap())
                        .unwrap();
                f(parsed);
            },
        ));
    }

    pub fn send(&self, cmd: Command) {
        self.socket
            .send_with_str(&serde_json::to_string(&cmd).unwrap())
            .unwrap();
    }
}
