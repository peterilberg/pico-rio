#![no_std]

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
pub enum Command {
    Ping,
    Restart,

    Subscribe,
    Unsubscribe,

    SetDO { pin: u8, value: bool },
    SetAO { pin: u8, value: u8 },
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
pub struct Info {
    pub content: Content,
    pub diagnostics: Diagnostics,
}

pub const NUM_PINS_DI: usize = 4;
pub const NUM_PINS_DO: usize = 5;
pub const NUM_PINS_AI: usize = 3;
pub const NUM_PINS_AO: usize = 2;

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
pub enum Content {
    Pong,

    DI { pins: [(u8, bool); NUM_PINS_DI] },
    DO { pins: [(u8, bool); NUM_PINS_DO] },
    AI { pins: [(u8, u8); NUM_PINS_AI] },
    AO { pins: [(u8, u8); NUM_PINS_AO] },
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
pub struct Diagnostics {
    pub timestamp_us: u64,
    pub execution_us: u64,
    pub jitter_in_us: u64,
    pub period_in_us: u64,
}
