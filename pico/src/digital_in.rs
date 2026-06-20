use embassy_rp::gpio::{Input, Level};
use embassy_time::Duration;
use messages::{Content, Pins};
use {defmt_rtt as _, panic_probe as _};

use crate::measurements;
use crate::network;
use crate::outbound;
use crate::timer::Timer;
use crate::watchdog;

#[embassy_executor::task]
pub async fn task(interval: Duration, pins: Pins<Input<'static>>) {
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

    let mut state = Pins::<bool>::new();
    for (pin, input) in pins {
        let _ = state.push((
            *pin,
            match input.get_level() {
                Level::Low => false,
                Level::High => true,
            },
        ));
    }

    measurements::set_digital(&state).await;
    outbound::send(Content::DI { pins: state }, timer.stop()).await;
}
