use std::time::{Duration, Instant};

use actix::{Actor, AsyncContext, StreamHandler, WrapFuture};
use actix_web::{
    dev::Server,
    web::{resource, Payload},
    App, HttpRequest, HttpResponse, HttpServer,
};
use actix_web_actors::ws;
use futures::Future;
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

fn ws_index(
    request: HttpRequest,
    payload: Payload,
) -> Result<HttpResponse, actix_web::Error> {
    println!("WS connected!");
    ws::start(WsSession::new(), &request, payload)
}

struct WsSession {}

impl WsSession {
    pub fn new() -> Self {
        Self {}
    }
}

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
            _ => {
                unimplemented!();
            }
        }
    }
}
