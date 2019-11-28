use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize, Hash, Eq, PartialEq)]
#[serde(tag = "command", content = "data")]
pub enum Event {
    RoomMuted { video: bool, audio: bool },
    RoomUnmuted { video: bool, audio: bool },
}

#[derive(Clone, Debug, Deserialize, Serialize, Hash, Eq, PartialEq)]
#[serde(tag = "command", content = "data")]
pub enum Command {
    MuteRoom { video: bool, audio: bool },
    UnmuteRoom { video: bool, audio: bool },
}
