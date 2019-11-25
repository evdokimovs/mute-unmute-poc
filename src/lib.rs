mod event_listener;
mod ws;

use std::{cell::RefCell, collections::HashMap, rc::Rc};

use js_sys::{Function, Promise};
use mute_unmute_poc_proto::{Command, Event};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::{spawn_local, JsFuture};

use crate::ws::WebSocket;

#[derive(Eq, PartialEq, Hash)]
struct PeerId(pub i32);

struct PromiseResolver {
    resolve: Option<Function>,
    reject: Option<Function>,
}

impl PromiseResolver {
    pub fn new_promise() -> (PromiseResolver, Promise) {
        let mut promise_resolver = PromiseResolver {
            resolve: None,
            reject: None,
        };
        let promise =
            Promise::new(&mut |resolve: Function, reject: Function| {
                promise_resolver.resolve = Some(resolve);
                promise_resolver.reject = Some(reject);
            });

        (promise_resolver, promise)
    }

    pub fn resolve(self) {
        self.resolve.unwrap().call0(&JsValue::NULL);
    }

    pub fn reject(self) {
        self.reject.unwrap().call0(&JsValue::NULL);
    }
}

struct Room {
    peers: HashMap<PeerId, PeerConnection>,
    on_mute: HashMap<MuteSubscriber, Vec<PromiseResolver>>,
    ws: WebSocket,
}

impl Room {
    pub fn handle_event(&mut self, event: Event) {
        match event {
            Event::RoomMuted { video, audio } => {
                let sub = MuteSubscriber { video, audio };
                self.peers.iter_mut().for_each(|(id, peer)| {
                    peer.mute(video, audio);
                });
                self.on_mute.remove(&sub).map(|q| {
                    q.into_iter().for_each(|resolver| {
                        resolver.resolve();
                    })
                });
            }
        }
    }
}

/// Async function which resolves after provided number of milliseconds.
pub async fn resolve_after(delay_ms: i32) -> Result<(), JsValue> {
    JsFuture::from(Promise::new(&mut |yes, _| {
        web_sys::window()
            .unwrap()
            .set_timeout_with_callback_and_timeout_and_arguments_0(
                &yes, delay_ms,
            )
            .unwrap();
    }))
    .await?;

    Ok(())
}

#[derive(Hash, PartialEq, Eq, Debug)]
struct MuteSubscriber {
    video: bool,
    audio: bool,
}

#[wasm_bindgen]
#[derive(Clone)]
pub struct RoomHandle(Rc<RefCell<Room>>);

#[wasm_bindgen]
impl RoomHandle {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        console_error_panic_hook::set_once();
        let ws = WebSocket::new("ws://127.0.0.1:10000/ws");
        let room = Rc::new(RefCell::new(Room {
            peers: HashMap::new(),
            on_mute: HashMap::new(),
            ws,
        }));
        let room_clone = room.clone();
        room.borrow_mut().ws.on_message(move |event| {
            room_clone.borrow_mut().handle_event(event);
        });
        Self(room)
    }

    pub fn mute(&self, audio: bool, video: bool) -> Promise {
        let (resolver, promise) = PromiseResolver::new_promise();
        let mute_subscriber = MuteSubscriber { audio, video };
        self.0
            .borrow_mut()
            .on_mute
            .entry(mute_subscriber)
            .or_insert_with(|| Vec::new())
            .push(resolver);
        let self_clone = self.clone();
        spawn_local(async move {
            self_clone
                .0
                .borrow()
                .ws
                .send(Command::MuteRoom { audio, video });
        });
        promise
    }
}

struct PeerConnection {
    tracks: Vec<Sender>,
}

impl PeerConnection {
    pub fn mute(&mut self, audio: bool, video: bool) {
        if audio {
            self.tracks
                .iter()
                .filter(|sender| &sender.kind == &SenderKind::Audio)
                .for_each(|sender| sender.mute());
        }
        if video {
            self.tracks
                .iter()
                .filter(|sender| &sender.kind == &SenderKind::Video)
                .for_each(|sender| sender.mute())
        }
    }
}

#[derive(Eq, PartialEq)]
enum SenderKind {
    Video,
    Audio,
}

struct Sender {
    kind: SenderKind,
}

impl Sender {
    pub fn mute(&self) {}
}
