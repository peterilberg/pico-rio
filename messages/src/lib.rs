#![no_std]

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
pub enum Command {
    Ping,
    Restart,

    Subscribe,
    Unsubscribe,

    SetDO { pin: u8, value: bool },
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
pub enum Notification {
    Pong,

    DO { pins: [(u8, bool); 1] },
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
pub struct Diagnostics {
    pub timestamp_us: u64,
    pub execution_us: u64,
    pub jitter_in_us: u64,
    pub period_in_us: u64,
}
