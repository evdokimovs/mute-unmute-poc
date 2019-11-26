//! Simple WebSocket server which sends [`Event::RoomMuted`] 3 seconds after
//! receives [`Command::MuteRoom`].

use std::time::Duration;

use actix::{Actor, AsyncContext, StreamHandler};
use actix_web::{
    dev::Server,
    web::{resource, Payload},
    App, HttpRequest, HttpResponse, HttpServer,
};
use actix_web_actors::ws;
use mute_unmute_poc_proto::{Command, Event};

fn main() {
    let sys = actix::System::new("control-api-mock");
    run();
    sys.run().unwrap();
}

fn run() -> Server {
    HttpServer::new(move || {
        App::new()
            .service(resource("/ws").route(actix_web::web::get().to(ws_index)))
    })
    .bind("0.0.0.0:10000")
    .unwrap()
    .start()
}

#[allow(clippy::needless_pass_by_value)]
fn ws_index(
    request: HttpRequest,
    payload: Payload,
) -> Result<HttpResponse, actix_web::Error> {
    println!("WS connected!");
    ws::start(WsSession, &request, payload)
}

struct WsSession;

impl Actor for WsSession {
    type Context = ws::WebsocketContext<Self>;
}

impl StreamHandler<ws::Message, ws::ProtocolError> for WsSession {
    fn handle(&mut self, msg: ws::Message, ctx: &mut Self::Context) {
        match msg {
            ws::Message::Text(text) => {
                let msg: Command = serde_json::from_str(&text).unwrap();
                println!("Getted {:?} from client.", msg);
                match msg {
                    Command::MuteRoom { video, audio } => {
                        ctx.run_later(Duration::from_secs(3), move |_, ctx| {
                            ctx.text(
                                serde_json::to_string(&Event::RoomMuted {
                                    video,
                                    audio,
                                })
                                .unwrap(),
                            );
                        });
                    }
                }
            }
            ws::Message::Close(_) => {
                println!("WebSocket closed.");
            }
            _ => {
                unimplemented!();
            }
        }
    }
}
