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

    BarGraph { pin: u8 },

    BangBangStart,
    BangBangStop,

    BangBangInput { pin: u8 },
    BangBangOutput { pin: u8 },

    BangBangLowerLimit { value: u8 },
    BangBangUpperLimit { value: u8 },
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
pub struct Info {
    pub content: Content,
    pub diagnostics: Diagnostics,
}

pub const NUM_PINS: usize = 32;
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

    BangBang { settings: BangBang },
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Clone, Copy)]
pub enum Mode {
    Off,
    Running,
    Waiting,
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Clone)]
pub struct BangBang {
    pub mode: Mode,
    pub input: u8,
    pub output: u8,
    pub lower_limit: u8,
    pub upper_limit: u8,
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
pub struct Diagnostics {
    pub timestamp_us: u64,
    pub execution_us: u64,
    pub jitter_in_us: u64,
    pub period_in_us: u64,
}
