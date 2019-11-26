//! Implementation for mute functional.

mod event_listener;
mod ws;

use std::{cell::RefCell, collections::HashMap, future::Future, rc::Rc};

use futures::channel::oneshot;
use js_sys::Promise;
use mute_unmute_poc_proto::{Command, Event};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::future_to_promise;

use crate::ws::WebSocket;

#[derive(Eq, PartialEq, Hash)]
struct PeerId(pub i32);

struct Room {
    peers: HashMap<PeerId, PeerConnection>,
    ws: WebSocket,
}

impl Room {
    pub fn handle_event(&mut self, event: &Event) {
        match event {
            Event::RoomMuted { video, audio } => {
                self.peers.iter_mut().for_each(|(_, peer)| {
                    peer.mute(*audio, *video);
                });
            }
        }
    }
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
            room_clone.borrow_mut().handle_event(&event);
        });
        Self(room)
    }

    // TODO: Maybe add timeout for this `Promise`?
    //       Also we can mute room without server's event if this promise is
    //       timed out.
    pub fn mute(&self, audio: bool, video: bool) -> Promise {
        let on_mute_fut: Vec<_> = self
            .0
            .borrow_mut()
            .peers
            .iter_mut()
            .map(|(_, peer)| peer.on_mute(audio, video))
            .collect();

        self.0.borrow().ws.send(Command::MuteRoom { audio, video });
        future_to_promise(async move {
            futures::future::join_all(on_mute_fut).await;
            Ok(JsValue::NULL)
        })
    }
}

impl Default for RoomHandle {
    fn default() -> Self {
        Self::new()
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

    pub fn filter_tracks_by_kind_mut(
        &mut self,
        audio: bool,
        video: bool,
    ) -> impl Iterator<Item = &mut Sender> {
        self.tracks.iter_mut().filter(move |sender| {
            (sender.kind == SenderKind::Audio && audio)
                || (sender.kind == SenderKind::Video && video)
        })
    }

    pub fn mute(&mut self, audio: bool, video: bool) {
        self.filter_tracks_by_kind_mut(audio, video)
            .for_each(Sender::mute);
    }

    pub fn on_mute(
        &mut self,
        audio: bool,
        video: bool,
    ) -> impl Future<Output = Vec<Result<(), oneshot::Canceled>>> {
        futures::future::join_all(
            self.filter_tracks_by_kind_mut(audio, video)
                .map(Sender::on_mute),
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
