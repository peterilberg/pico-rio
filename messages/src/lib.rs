#![no_std]

use heapless::{String, Vec};
use serde::{Deserialize, Serialize};

pub const PICO_ADDRESS_ENVNAME: &str = "PICO_ADDRESS";
pub const PICO_ADDRESS_DEFAULT: &str = "192.168.7.1:1234";
pub const PICO_ADDRESS_BUILD_TIME: Option<&str> = option_env!("PICO_ADDRESS");

pub type Pins<T> = Vec<(u8, T), 8>;
pub type Label = String<16>;

pub const MAX_NUM_MEASUREMENTS: usize = 32;
pub type Measurements = [u8; MAX_NUM_MEASUREMENTS];

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
pub enum Command {
    Ping,
    Restart,

    Subscribe,

    SetDO { pin: u8, value: bool },
    SetAO { pin: u8, value: u8 },

    BangBangStart,
    BangBangStop,

    BangBangInput { pin: u8 },
    BangBangOutput { pin: u8 },

    BangBangLowerLimit { value: u8 },
    BangBangUpperLimit { value: u8 },

    BangBangShow,
    BangBangHide,

    ClearDisplay,
    AddLine { label: Label, value: Value },
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
pub struct Info {
    pub content: Content,
    pub diagnostics: Diagnostics,
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
pub enum Content {
    Pong,

    DI { pins: Pins<bool> },
    DO { pins: Pins<bool> },

    AI { pins: Pins<u8> },
    AO { pins: Pins<u8> },

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
    pub input_pin: u8,
    pub output_pin: u8,
    pub lower_limit: u8,
    pub upper_limit: u8,
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug)]
pub enum Value {
    None,
    OffOn(u8),
    OnOff(u8),
    Analog(u8),
    Number(u8),
    Boolean(bool),
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
pub struct Diagnostics {
    pub timestamp_us: u64,
    pub execution_us: u64,
    pub jitter_in_us: u64,
    pub period_in_us: u64,
}
