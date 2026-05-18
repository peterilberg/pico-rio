#![no_std]

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
pub enum Kind {
    Status,
    Echo([u8; 32]),
    DigitalOut { pin_25: DigitalLevel },
}

// TODO change times to microseconds
#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
pub struct Message {
    pub sender: u32,
    pub time_ms: u64,
    pub exec_ms: u64,
    pub jitter_ms: u64,
    pub period_ms: u64,
    pub kind: Kind,
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Clone, Copy)]
pub enum DigitalLevel {
    Off,
    On,
}

pub fn add(left: u64, right: u64) -> u64 {
    left + right
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
