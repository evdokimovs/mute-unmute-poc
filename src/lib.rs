mod event_listener;
mod ws;

use std::{cell::RefCell, collections::HashMap, rc::Rc};

use futures::channel::{mpsc, oneshot};
use js_sys::{Function, Promise};
use mute_unmute_poc_proto::{Command, Event};
use std::pin::Pin;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::{spawn_local, JsFuture};

use crate::ws::WebSocket;
use std::future::Future;

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
    ws: WebSocket,
}

impl Room {
    pub fn handle_event(&mut self, event: Event) {
        match event {
            Event::RoomMuted { video, audio } => {
                self.peers.iter_mut().for_each(|(id, peer)| {
                    peer.mute(audio, video);
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
        let mut peers = HashMap::new();
        peers.insert(PeerId(100), PeerConnection::new());
        let room = Rc::new(RefCell::new(Room { peers, ws }));
        let room_clone = room.clone();
        room.borrow_mut().ws.on_message(move |event| {
            room_clone.borrow_mut().handle_event(event);
        });
        Self(room)
    }

    pub fn mute(&self, audio: bool, video: bool) -> Promise {
        let (resolver, promise) = PromiseResolver::new_promise();
        let qq: Vec<_> = self
            .0
            .borrow_mut()
            .peers
            .iter_mut()
            .map(|(_, peer)| peer.on_mute(audio, video))
            .collect();

        spawn_local(async move {
            futures::future::join_all(qq).await;
            resolver.resolve();
        });

        self.0.borrow().ws.send(Command::MuteRoom { audio, video });
        promise
    }
}

struct PeerConnection {
    tracks: Vec<Sender>,
}

impl PeerConnection {
    pub fn new() -> Self {
        Self {
            tracks: vec![
                Sender::new(SenderKind::Video),
                Sender::new(SenderKind::Audio),
            ],
        }
    }

    pub fn mute(&mut self, audio: bool, video: bool) {
        self.tracks
            .iter_mut()
            .filter(|sender| {
                (&sender.kind == &SenderKind::Audio && audio)
                    || (&sender.kind == &SenderKind::Video && video)
            })
            .for_each(|sender| sender.mute());
    }

    pub fn on_mute(
        &mut self,
        audio: bool,
        video: bool,
    ) -> impl Future<Output = Vec<Result<(), oneshot::Canceled>>> {
        futures::future::join_all(
            self.tracks
                .iter_mut()
                .filter(|sender| {
                    (&sender.kind == &SenderKind::Audio && audio)
                        || (&sender.kind == &SenderKind::Video && video)
                })
                .map(|sender| sender.on_mute()),
        )
    }
}

#[derive(Debug, Eq, PartialEq)]
enum SenderKind {
    Video,
    Audio,
}

#[derive(Debug)]
struct Sender {
    kind: SenderKind,
    on_mute: Vec<oneshot::Sender<()>>,
}

impl Sender {
    pub fn new(kind: SenderKind) -> Self {
        Self {
            kind,
            on_mute: Vec::new(),
        }
    }

    pub fn mute(&mut self) {
        self.on_mute
            .drain(..)
            .for_each(|on_mute| on_mute.send(()).unwrap());
    }

    pub fn on_mute(
        &mut self,
    ) -> impl Future<Output = Result<(), oneshot::Canceled>> {
        let (tx, rx) = oneshot::channel();
        self.on_mute.push(tx);
        rx
    }
}
