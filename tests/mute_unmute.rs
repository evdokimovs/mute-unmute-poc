use futures::{
    channel::{mpsc, oneshot},
    future::Either,
    StreamExt as _,
};
use mute_unmute_poc::{
    proto::Event, resolve_after, ws::MockRpcClient, RoomHandle,
};
use wasm_bindgen_futures::spawn_local;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
async fn mute_unmute() {
    let room_handle = RoomHandle::new();
    let (test_tx, test_rx) = oneshot::channel();
    spawn_local(async move {
        room_handle.inner_mute(true, true).await;
        test_tx.send(()).unwrap();
    });
    let res = futures::future::select(
        Box::pin(test_rx),
        Box::pin(resolve_after(3500)),
    )
    .await;
    match res {
        Either::Left(_) => (),
        Either::Right(_) => panic!(),
    }
}

#[wasm_bindgen_test]
async fn mute_many_times() {
    let mut ws = MockRpcClient::new();
    ws.expect_on_message().return_once(|| {
        Box::pin(futures::stream::once(async {
            resolve_after(500).await.unwrap();
            Event::RoomMuted {
                audio: true,
                video: true,
            }
        }))
    });
    ws.expect_send().return_once(|_| {});
    let room_handle = RoomHandle::new_with_client(Box::new(ws));
    let (test_tx, test_rx) = oneshot::channel();
    spawn_local(async move {
        let mut futs = Vec::new();
        for _ in 0..10 {
            futs.push(room_handle.inner_mute(true, true));
        }
        futures::future::join_all(futs).await;
        test_tx.send(()).unwrap();
    });

    let res = futures::future::select(
        Box::pin(test_rx),
        Box::pin(resolve_after(3500)),
    )
    .await;
    match res {
        Either::Left(_) => (),
        Either::Right(_) => panic!(),
    }
}

#[wasm_bindgen_test]
async fn unmute_when_room_not_muted() {
    let mut ws = MockRpcClient::new();
    ws.expect_on_message()
        .return_once(|| futures::stream::pending().boxed());
    let room_handle = RoomHandle::new_with_client(Box::new(ws));
    let (test_tx, test_rx) = oneshot::channel();
    spawn_local(async move {
        room_handle.inner_unmute(true, true).await;
        test_tx.send(()).unwrap();
    });

    let res = futures::future::select(
        Box::pin(test_rx),
        Box::pin(resolve_after(3500)),
    )
    .await;
    match res {
        Either::Left(_) => (),
        Either::Right(_) => panic!(),
    }
}
