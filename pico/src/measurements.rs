use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use messages::NUM_PINS;
use {defmt_rtt as _, panic_probe as _};

use crate::bang_bang;
use crate::bar_graph;
use crate::network;

type Message = [Option<u8>; NUM_PINS];

pub type Measurements = [u8; NUM_PINS];

static INBOX: Channel<CriticalSectionRawMutex, Message, 16> = Channel::new();

pub async fn set_analog(pins: &[(u8, u8)]) {
    send(pins, |value| *value).await;
}

pub async fn set_digital(pins: &[(u8, bool)]) {
    send(pins, |value| if *value { 100 } else { 0 }).await;
}

#[embassy_executor::task]
pub async fn task() {
    network::wait_for_network().await;

    let mut measurements = [0_u8; NUM_PINS];

    loop {
        let updates = INBOX.receive().await;

        let mut recorded_new_measurement = false;
        for (i, update) in updates.into_iter().enumerate() {
            if let Some(value) = update
                && value != measurements[i]
            {
                measurements[i] = value;
                recorded_new_measurement = true;
            }
        }

        if recorded_new_measurement {
            bar_graph::notify(&measurements).await;
            bang_bang::notify(&measurements).await;
        }
    }
}

async fn send<T>(pins: &[(u8, T)], f: impl Fn(&T) -> u8) {
    let mut measurements = [None; NUM_PINS];
    for (pin, value) in pins.iter() {
        let pin = *pin as usize;
        if pin >= NUM_PINS {
            continue;
        }
        measurements[pin] = Some(f(value));
    }
    INBOX.send(measurements).await;
}
