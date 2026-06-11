use embassy_futures::select::{Either, select};
use embassy_rp::pwm::{Config, Pwm, SetDutyCycle};
use embassy_time::Duration;
use messages::{Content, NUM_PINS_AO};
use serde::{Deserialize, Serialize};
use {defmt_rtt as _, panic_probe as _};

use crate::mailbox::{Inbox, Outbox};
use crate::network;
use crate::outbound;
use crate::timer::Timer;
use crate::watchdog;

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
pub enum Message {
    Set { pin: u8, value: u8 },
}

pub fn pwm_configuation(frequency_hz: u32) -> Config {
    let clock_freq_hz = embassy_rp::clocks::clk_sys_freq();
    let divider = 16u8;
    let period = (clock_freq_hz / (frequency_hz * divider as u32)) as u16 - 1;

    let mut config = Config::default();
    config.top = period;
    config.divider = divider.into();
    config
}

#[embassy_executor::task]
pub async fn task(
    interval: Duration,
    pins: [(u8, u8, Pwm<'static>); NUM_PINS_AO],
    inbox: Inbox<Message>,
    outbound: Outbox<outbound::Message>,
) {
    network::wait_for_network().await;

    let mut pins = pins;
    let mut timer = Timer::new(interval);
    loop {
        match select(timer.wait(), inbox.receive()).await {
            Either::First(()) => {
                send_duty_cycles(&pins, outbound, &mut timer).await;
            }
            Either::Second(Message::Set { pin, value }) => {
                set_duty_cycle(pin, value, &mut pins);
            }
        }

        watchdog::notify();
    }
}

async fn send_duty_cycles(
    pins: &[(u8, u8, Pwm<'static>)],
    outbound: Outbox<outbound::Message>,
    timer: &mut Timer,
) {
    timer.start();

    let mut state = [(0_u8, 0_u8); NUM_PINS_AO];
    for (i, (pin, value, _output)) in pins.iter().enumerate() {
        state[i].0 = *pin;
        state[i].1 = *value;
    }

    outbound
        .send(outbound::Message {
            content: Content::AO { pins: state },
            diagnostics: timer.stop(),
        })
        .await;
}

fn set_duty_cycle(pin: u8, value: u8, pins: &mut [(u8, u8, Pwm<'static>)]) {
    let Some((_, duty_cycle, output)) = pins.iter_mut().find(|(known_pin, _, _)| pin == *known_pin)
    else {
        log::info!("analog_out: ignore unknown pin {}", pin);
        return;
    };

    *duty_cycle = value.clamp(0, 100);
    let result = if *duty_cycle == 0 {
        output.set_duty_cycle_fully_off()
    } else {
        output.set_duty_cycle_percent(*duty_cycle)
    };

    if let Err(error) = result {
        log::info!(
            "analog_out: duty cycle {} on pin {} failed: {:?}",
            duty_cycle,
            pin,
            error
        );
    }
}
