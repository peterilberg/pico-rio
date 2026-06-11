use embassy_rp::gpio::{Input, Level};
use embassy_time::Duration;
use messages::{Content, NUM_PINS_DI};
use {defmt_rtt as _, panic_probe as _};

use crate::network;
use crate::outbound;
use crate::timer::Timer;
use crate::watchdog;

#[embassy_executor::task]
pub async fn task(interval: Duration, pins: [(u8, Input<'static>); NUM_PINS_DI]) {
    network::wait_for_network().await;

    let mut timer = Timer::new(interval);
    loop {
        timer.wait().await;
        send_pin_levels(&pins, &mut timer).await;

        watchdog::notify();
    }
}

async fn send_pin_levels(pins: &[(u8, Input<'static>)], timer: &mut Timer) {
    timer.start();

    let mut state = [(0_u8, false); NUM_PINS_DI];
    for (i, (pin, input)) in pins.iter().enumerate() {
        state[i].0 = *pin;
        state[i].1 = match input.get_level() {
            Level::Low => false,
            Level::High => true,
        };
    }

    outbound::send(Content::DI { pins: state }, timer.stop()).await;
}
