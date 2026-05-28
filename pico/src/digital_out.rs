use embassy_futures::select::{Either, select};
use embassy_rp::gpio::{Level, Output};
use embassy_time::Duration;
use messages::Content;
use serde::{Deserialize, Serialize};
use {defmt_rtt as _, panic_probe as _};

use crate::mailbox::{Inbox, Outbox};
use crate::network;
use crate::outbound;
use crate::timer::Timer;
use crate::watchdog;

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
pub enum Message {
    Set { pin: u8, value: bool },
}

#[embassy_executor::task]
pub async fn task(
    interval: Duration,
    pins: [(u8, Output<'static>); 1],
    inbox: Inbox<Message>,
    outbound: Outbox<outbound::Message>,
) {
    network::wait_for_network().await;

    let mut pins = pins;
    let mut timer = Timer::new(interval);
    loop {
        match select(timer.wait(), inbox.receive()).await {
            Either::First(()) => {
                send_pin_levels(&pins, outbound, &mut timer).await;
            }
            Either::Second(Message::Set { pin, value }) => {
                set_pin(pin, value, &mut pins);
            }
        }

        watchdog::notify();
    }
}

async fn send_pin_levels(
    pins: &[(u8, Output<'static>); 1],
    outbound: Outbox<outbound::Message>,
    timer: &mut Timer,
) {
    timer.start();

    let mut state = [(0_u8, false); 1];
    for (i, (pin, output)) in pins.iter().enumerate() {
        state[i].0 = *pin;
        state[i].1 = match output.get_output_level() {
            Level::Low => false,
            Level::High => true,
        };
    }

    outbound
        .send(outbound::Message {
            content: Content::DO { pins: state },
            diagnostics: timer.stop(),
        })
        .await;
}

fn set_pin(pin: u8, value: bool, pins: &mut [(u8, Output<'static>); 1]) {
    match pins.iter_mut().find(|(known_pin, _)| pin == *known_pin) {
        None => log::info!("digital_out: ignore unknown pin {}", pin),
        Some((_, output)) => {
            output.set_level(match value {
                false => Level::Low,
                true => Level::High,
            });
        }
    }
}
