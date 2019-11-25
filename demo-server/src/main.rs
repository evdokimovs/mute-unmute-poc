use actix::{Actor, StreamHandler};
use actix_web_actors::ws;
use actix_web::{HttpRequest, HttpResponse, HttpServer};
use actix_web::dev::{Server};
use actix_web::web::Payload;
use actix_web::App;
use futures::Future;
use actix_web::web::resource;
use serde::{Serialize, Deserialize};
use mute_unmute_poc_proto::{Event, Command};

fn main() {
    let sys = actix::System::new("control-api-mock");
    run();
    sys.run().unwrap();
}

fn run() -> Server {
    HttpServer::new(move || {
        App::new()
            .service(
                resource("/ws")
                    .route(actix_web::web::get().to(ws_index))
            )
    })
        .bind("0.0.0.0:10000")
        .unwrap()
        .start()
}

fn ws_index(
    request: HttpRequest,
    payload: Payload,
) -> Result<HttpResponse, actix_web::Error> {
    ws::start(
        WsSession::new(),
        &request,
        payload,
    )
}

struct WsSession {}

impl WsSession {
    pub fn new() -> Self {
        Self {

        }
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
                match msg {
                    Command::MuteRoom { video, audio } => {
                        ctx.text(serde_json::to_string(&Event::RoomMuted {
                            video,
                            audio,
                        }).unwrap());
                    }
                }
            }
            _ => {
                unimplemented!();
            }
        }
    }
}
