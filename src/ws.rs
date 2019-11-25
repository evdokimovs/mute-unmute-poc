use std::rc::Rc;

use mute_unmute_poc_proto::{Command, Event};
use wasm_bindgen_futures::spawn_local;
use web_sys::WebSocket as SysWebSocket;

use crate::{event_listener::EventListener, resolve_after};

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
        let socket_clone = Rc::clone(&self.socket);
        spawn_local(async move {
            while socket_clone.ready_state() == 0 {
                resolve_after(50).await;
            }
            socket_clone
                .send_with_str(&serde_json::to_string(&cmd).unwrap())
                .unwrap();
        });
    }
}
