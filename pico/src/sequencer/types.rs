use embassy_time::Duration;
use messages::{Command, Value};
use {defmt_rtt as _, panic_probe as _};

use crate::measurements::Measurements;

pub type Condition = fn(&Measurements) -> bool;

pub struct Guard {
    pub id: u8,
    pub condition: Condition,
}

pub struct Sequence<'seq> {
    pub id: u8,
    pub guards: &'seq [Guard],
    pub steps: &'seq [Step<'seq>],
    pub abort: &'seq [Step<'seq>],
}

pub enum Step<'step> {
    At(Duration),
    Test {
        deadline: Duration,
        condition: Condition,
    },
    Do(Command),

    /// For static construction of sequences.
    AddLine {
        label: &'step str,
        value: Value,
    },
}
