use embassy_futures::select::{Either, select};
use embassy_rp::pwm::{Config, Pwm, SetDutyCycle};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embassy_time::Duration;
use messages::{Content, Pins};
use {defmt_rtt as _, panic_probe as _};

use crate::measurements;
use crate::network;
use crate::outbound;
use crate::timer::Timer;
use crate::watchdog;

enum Message {
    Set { pin: u8, value: u8 },
}

static INBOX: Channel<CriticalSectionRawMutex, Message, 16> = Channel::new();

pub async fn set_pin(pin: u8, value: u8) {
    INBOX.send(Message::Set { pin, value }).await;
}

pub fn configuation(frequency_hz: u32) -> Config {
    let clock_freq_hz = embassy_rp::clocks::clk_sys_freq();
    let divider = 16u8;
    let period = (clock_freq_hz / (frequency_hz * divider as u32)) as u16 - 1;

    let mut config = Config::default();
    config.top = period;
    config.divider = divider.into();
    config
}

#[embassy_executor::task]
pub async fn task(interval: Duration, pins: Pins<Pwm<'static>>) {
    network::wait_for_network().await;

    let mut pins: Pins<Output> = Pins::from_iter(
        pins.into_iter()
            .map(|(pin, pwm)| (pin, Output::new(pin, pwm))),
    );

    let mut timer = Timer::new(interval);
    loop {
        match select(timer.wait(), INBOX.receive()).await {
            Either::First(()) => {
                send_duty_cycles(&pins, &mut timer).await;
            }
            Either::Second(Message::Set { pin, value }) => {
                if let Some(output) = get_output(pin, &mut pins) {
                    output.set_duty_cycle(value);
                }
            }
        }

        watchdog::notify();
    }
}

async fn send_duty_cycles(pins: &Pins<Output>, timer: &mut Timer) {
    timer.start();

    let mut state = Pins::<u8>::new();
    for (_, output) in pins {
        let _ = state.push((output.pin, output.duty_cycle));
    }

    measurements::set_analog(&state).await;
    outbound::send(Content::AO { pins: state }, timer.stop()).await;
}

fn get_output(pin: u8, pins: &mut Pins<Output>) -> Option<&mut Output> {
    pins.iter_mut()
        .find(|(known_pin, _)| pin == *known_pin)
        .map(|(_, output)| output)
}

struct Output {
    pin: u8,
    duty_cycle: u8,
    pwm: Pwm<'static>,
}

impl Output {
    fn new(pin: u8, pwm: Pwm<'static>) -> Self {
        Output {
            pin,
            duty_cycle: 0,
            pwm,
        }
    }

    fn set_duty_cycle(&mut self, value: u8) {
        self.duty_cycle = value.clamp(0, 100);
        let result = if self.duty_cycle == 0 {
            self.pwm.set_duty_cycle_fully_off()
        } else {
            self.pwm.set_duty_cycle_percent(self.duty_cycle)
        };

        if let Err(error) = result {
            log::info!(
                "analog_out: duty cycle {} on pin {} failed: {:?}",
                self.duty_cycle,
                self.pin,
                error
            );
        }
    }
}
