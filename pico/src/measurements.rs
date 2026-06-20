use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use {defmt_rtt as _, panic_probe as _};

use messages::MAX_NUM_MEASUREMENTS;

use crate::bang_bang;
use crate::display;
use crate::network;

pub type Measurements = messages::Measurements;

type Message = heapless::Vec<(u8, u8), MAX_NUM_MEASUREMENTS>;

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

    let mut measurements = Measurements::default();

    loop {
        let updates = INBOX.receive().await;

        let mut recorded_new_measurement = false;
        for (i, measurement) in updates.into_iter() {
            let i = i as usize;
            if measurement != measurements[i] {
                measurements[i] = measurement;
                recorded_new_measurement = true;
            }
        }

        if recorded_new_measurement {
            display::notify(&measurements).await;
            bang_bang::notify(&measurements).await;
        }
    }
}

async fn send<T>(pins: &[(u8, T)], f: impl Fn(&T) -> u8) {
    let measurements = Message::from_iter(
        pins.iter()
            .filter(|(pin, _)| (*pin as usize) < MAX_NUM_MEASUREMENTS)
            .map(|(pin, value)| (*pin, f(value))),
    );
    INBOX.send(measurements).await;
}
