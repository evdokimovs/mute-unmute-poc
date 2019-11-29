//! Implementation for mute functional.

pub mod proto;
pub mod ws;

use std::{cell::RefCell, collections::HashMap, future::Future, rc::Rc};

use futures::{channel::oneshot, StreamExt as _};
use js_sys::Promise;
use proto::{Command, Event};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::{future_to_promise, spawn_local, JsFuture};

use crate::ws::{RpcClient, WebSocket};

/// Resolves after provided number of milliseconds.
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

#[derive(Eq, PartialEq, Hash)]
struct PeerId(pub i32);

struct Room {
    peers: HashMap<PeerId, PeerConnection>,
    ws: Box<dyn RpcClient>,
}

impl Room {
    pub fn handle_event(&mut self, event: &Event) {
        match event {
            Event::RoomMuted { video, audio } => {
                self.peers.iter_mut().for_each(|(_, peer)| {
                    peer.mute(*audio, *video);
                });
            }
            Event::RoomUnmuted { video, audio } => {
                self.peers.iter_mut().for_each(|(_, peer)| {
                    peer.unmute(*audio, *video);
                })
            }
        }
    }

    fn is_busy(&self, audio: bool, video: bool) -> bool {
        self.peers
            .values()
            .filter(|peer| peer.is_busy(audio, video))
            .count()
            != 0
    }
}

#[wasm_bindgen]
#[derive(Clone)]
pub struct RoomHandle(Rc<RefCell<Room>>);

impl RoomHandle {
    pub fn new_with_client(ws: Box<dyn RpcClient>) -> Self {
        console_error_panic_hook::set_once();
        let mut peers = HashMap::new();
        peers.insert(PeerId(100), PeerConnection::new());
        let room = Rc::new(RefCell::new(Room { peers, ws }));
        let room_clone = room.clone();
        let mut fut = room.borrow_mut().ws.on_message();
        spawn_local(async move {
            while let Some(event) = fut.next().await {
                room_clone.borrow_mut().handle_event(&event);
            }
        });
        Self(room)
    }

    pub async fn inner_mute(&self, audio: bool, video: bool) {
        let is_room_busy = self.0.borrow().is_busy(audio, video);
        let on_mute_fut: Vec<_> = self
            .0
            .borrow_mut()
            .peers
            .iter_mut()
            .filter(|(_, peer)| !peer.is_muted(audio, video))
            .map(|(_, peer)| peer.on_mute(audio, video))
            .collect();

        if !is_room_busy && on_mute_fut.len() > 0 {
            self.0
                .borrow_mut()
                .ws
                .send(Command::MuteRoom { audio, video });
        }

        futures::future::join_all(on_mute_fut).await;
    }

    pub async fn inner_unmute(&self, audio: bool, video: bool) {
        let is_room_busy = self.0.borrow().is_busy(audio, video);
        let on_unmute_fut: Vec<_> = self
            .0
            .borrow_mut()
            .peers
            .iter_mut()
            .filter(|(_, peer)| peer.is_muted(audio, video))
            .map(|(_, peer)| peer.on_unmute(audio, video))
            .collect();

        if !is_room_busy && on_unmute_fut.len() > 0 {
            self.0
                .borrow()
                .ws
                .send(Command::UnmuteRoom { audio, video });
        }

        futures::future::join_all(on_unmute_fut).await;
    }
}

#[wasm_bindgen]
impl RoomHandle {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self::new_with_client(Box::new(WebSocket::new()))
    }

    // TODO: Maybe add timeout for this `Promise`?
    //       Also we can mute room without server's event if this promise is
    //       timed out.
    pub fn mute(&self, audio: bool, video: bool) -> Promise {
        let self_clone = self.clone();
        future_to_promise(async move {
            self_clone.inner_mute(audio, video).await;
            Ok(JsValue::NULL)
        })
    }

    pub fn unmute(&self, audio: bool, video: bool) -> Promise {
        let self_clone = self.clone();
        future_to_promise(async move {
            self_clone.inner_unmute(audio, video).await;
            Ok(JsValue::NULL)
        })
    }
}

impl Default for RoomHandle {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug)]
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

    pub fn filter_tracks_by_kind(
        &self,
        audio: bool,
        video: bool,
    ) -> impl Iterator<Item = &Sender> {
        self.tracks.iter().filter(move |sender| {
            (sender.kind == SenderKind::Audio && audio)
                || (sender.kind == SenderKind::Video && video)
        })
    }

    pub fn mute(&mut self, audio: bool, video: bool) {
        self.filter_tracks_by_kind_mut(audio, video)
            .filter(|sender| !sender.is_muted())
            .for_each(Sender::mute);
    }

    pub fn unmute(&mut self, audio: bool, video: bool) {
        self.filter_tracks_by_kind_mut(audio, video)
            .filter(|sender| sender.is_muted())
            .for_each(Sender::unmute);
    }

    pub fn on_mute(
        &mut self,
        audio: bool,
        video: bool,
    ) -> impl Future<Output = Vec<Result<(), oneshot::Canceled>>> {
        futures::future::join_all(
            self.filter_tracks_by_kind_mut(audio, video)
                .filter(|sender| !sender.is_muted())
                .map(Sender::on_mute),
        )
    }

    pub fn on_unmute(
        &mut self,
        audio: bool,
        video: bool,
    ) -> impl Future<Output = Vec<Result<(), oneshot::Canceled>>> {
        futures::future::join_all(
            self.filter_tracks_by_kind_mut(audio, video)
                .filter(|sender| sender.is_muted())
                .map(Sender::on_unmute),
        )
    }

    pub fn is_busy(&self, audio: bool, video: bool) -> bool {
        self.filter_tracks_by_kind(audio, video)
            .filter(|sender| !sender.is_busy())
            .count()
            == 0
    }

    pub fn is_muted(&self, audio: bool, video: bool) -> bool {
        self.filter_tracks_by_kind(audio, video)
            .filter(|sender| !sender.is_muted())
            .count()
            == 0
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
    on_unmute: Vec<oneshot::Sender<()>>,
    is_muted: bool,
}

impl Sender {
    pub fn new(kind: SenderKind) -> Self {
        Self {
            kind,
            on_mute: Vec::new(),
            on_unmute: Vec::new(),
            is_muted: false,
        }
    }

    pub fn mute(&mut self) {
        self.is_muted = true;
        self.on_mute
            .drain(..)
            .for_each(|on_mute| on_mute.send(()).unwrap());
    }

    pub fn unmute(&mut self) {
        self.is_muted = false;
        self.on_unmute
            .drain(..)
            .for_each(|on_unmute| on_unmute.send(()).unwrap());
    }

    pub fn on_mute(
        &mut self,
    ) -> impl Future<Output = Result<(), oneshot::Canceled>> {
        let (tx, rx) = oneshot::channel();
        self.on_mute.push(tx);
        rx
    }

    pub fn on_unmute(
        &mut self,
    ) -> impl Future<Output = Result<(), oneshot::Canceled>> {
        let (tx, rx) = oneshot::channel();
        self.on_unmute.push(tx);
        rx
    }

    pub fn is_busy(&self) -> bool {
        self.on_mute.len() > 0 || self.on_unmute.len() > 0
    }

    pub fn is_muted(&self) -> bool {
        self.is_muted
    }
}
