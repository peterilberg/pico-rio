use embassy_futures::select::{Either, select};
use embassy_rp::gpio::{Level, Output};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embassy_time::Duration;
use messages::{Content, NUM_PINS_DO};
use {defmt_rtt as _, panic_probe as _};

use crate::measurements;
use crate::network;
use crate::outbound;
use crate::timer::Timer;
use crate::watchdog;

enum Message {
    Set { pin: u8, value: bool },
}

static INBOX: Channel<CriticalSectionRawMutex, Message, 16> = Channel::new();

pub async fn set_pin(pin: u8, value: bool) {
    INBOX.send(Message::Set { pin, value }).await;
}

#[embassy_executor::task]
pub async fn task(interval: Duration, pins: [(u8, Output<'static>); NUM_PINS_DO]) {
    network::wait_for_network().await;

    let mut pins = pins;
    let mut timer = Timer::new(interval);
    loop {
        match select(timer.wait(), INBOX.receive()).await {
            Either::First(()) => {
                send_pin_levels(&pins, &mut timer).await;
            }
            Either::Second(Message::Set { pin, value }) => {
                let level = get_level(value);
                get_pin(pin, &mut pins).map(|pin| pin.set_level(level));
            }
        }

        watchdog::notify();
    }
}

async fn send_pin_levels(pins: &[(u8, Output<'static>)], timer: &mut Timer) {
    timer.start();

    let mut state = [(0_u8, false); NUM_PINS_DO];
    for (i, (pin, output)) in pins.iter().enumerate() {
        state[i].0 = *pin;
        state[i].1 = match output.get_output_level() {
            Level::Low => false,
            Level::High => true,
        };
    }

    measurements::set_digital(&state).await;
    outbound::send(Content::DO { pins: state }, timer.stop()).await;
}

fn get_level(value: bool) -> Level {
    match value {
        false => Level::Low,
        true => Level::High,
    }
}

fn get_pin<'pins>(
    pin: u8,
    pins: &'pins mut [(u8, Output<'static>)],
) -> Option<&'pins mut Output<'static>> {
    match pins.iter_mut().find(|(known_pin, _)| pin == *known_pin) {
        None => None,
        Some((_, output)) => Some(output),
    }
}
