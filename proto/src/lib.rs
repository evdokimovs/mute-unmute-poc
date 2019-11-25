use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "command", content = "data")]
pub enum Event {
    RoomMuted { video: bool, audio: bool },
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "command", content = "data")]
pub enum Command {
    MuteRoom { video: bool, audio: bool },
}
