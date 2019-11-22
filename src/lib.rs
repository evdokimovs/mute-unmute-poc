use std::{
    cell::RefCell,
    collections::HashMap,
    rc::{Rc, Weak},
};

use futures::{channel::oneshot, Future, TryFutureExt};
use js_sys::{Function, Promise};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::{spawn_local, JsFuture};

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
    on_mute: Vec<PromiseResolver>,
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

#[wasm_bindgen]
struct RoomHandle(Rc<RefCell<Room>>);

#[wasm_bindgen]
impl RoomHandle {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self(Rc::new(RefCell::new(Room {
            peers: HashMap::new(),
            on_mute: Vec::new(),
        })))
    }

    pub fn test(&mut self) -> Promise {
        let (tx, rx) = PromiseResolver::new_promise();
        self.0.borrow_mut().on_mute.push(tx);

        let inner_clone = self.0.clone();
        spawn_local(async move {
            resolve_after(1000).await;
            inner_clone.borrow_mut().on_mute.drain(..).for_each(
                |promise_resolver| {
                    promise_resolver.resolve();
                },
            );
        });

        rx
    }
}

struct PeerConnection {
    tracks: Vec<Track>,
}

struct Track {}
