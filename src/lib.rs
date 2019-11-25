use std::{
    cell::RefCell,
    collections::HashMap,
    rc::{Rc, Weak},
};

use futures::{channel::oneshot, Future, TryFutureExt};
use js_sys::{Function, Promise};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::{spawn_local, JsFuture};
use mute_unmute_poc_proto::{Event};

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
    on_mute: HashMap<MuteSubscriber, PromiseResolver>,
}

impl Room {
    pub fn handle_event(&mut self, event: Event) {
        match event {
            Event::RoomMuted { video, audio } => {
                let sub = MuteSubscriber {
                    video,
                    audio,
                };
                let resolver = self.on_mute.remove(&sub).unwrap();
                resolver.resolve();
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
struct RoomHandle(Rc<RefCell<Room>>);

#[wasm_bindgen]
impl RoomHandle {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self(Rc::new(RefCell::new(Room {
            peers: HashMap::new(),
            on_mute: HashMap::new(),
        })))
    }

    pub fn mute(&self, audio: bool, video: bool) -> Promise {
        let (resolver, promise) = PromiseResolver::new_promise();
        let mute_subscriber = MuteSubscriber {
            audio,
            video,
        };
        self.0.borrow_mut().on_mute.insert(mute_subscriber, resolver);
        promise
    }
}

struct PeerConnection {
    tracks: Vec<Sender>,
}

impl PeerConnection {
    pub fn mute(&mut self, kind: SenderKind) {
        self.tracks.iter()
            .filter(|sender| &sender.kind == &kind)
            .for_each(|sender| sender.mute());
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
    pub fn mute(&self) {

    }
}
