use embassy_rp::Peri;
use embassy_rp::adc::{Adc, Async, Channel, Config, InterruptHandler};
use embassy_rp::bind_interrupts;
use embassy_rp::peripherals::ADC;
use embassy_time::Duration;
use messages::{Content, NUM_PINS_AI};
use {defmt_rtt as _, panic_probe as _};

use crate::network;
use crate::outbound;
use crate::timer::Timer;
use crate::watchdog;

bind_interrupts!(
    struct Irqs {
        ADC_IRQ_FIFO => InterruptHandler;
    }
);

#[embassy_executor::task]
pub async fn task(
    interval: Duration,
    adc: Peri<'static, ADC>,
    mut pins: [(u8, Channel<'static>); NUM_PINS_AI],
) {
    network::wait_for_network().await;

    let mut adc = Adc::new(adc, Irqs, Config::default());
    let mut timer = Timer::new(interval);
    loop {
        timer.wait().await;
        send_adc_values(&mut adc, &mut pins, &mut timer).await;

        watchdog::notify();
    }
}

async fn send_adc_values(
    adc: &mut Adc<'_, Async>,
    pins: &mut [(u8, Channel<'static>)],
    timer: &mut Timer,
) {
    timer.start();

    let mut state = [(0_u8, 0_u8); NUM_PINS_AI];
    for (i, (pin, input)) in pins.iter_mut().enumerate() {
        state[i].0 = *pin;
        state[i].1 = match adc.read(input).await {
            Ok(value) => (value as f32 / 40.96) as u8,
            Err(_) => 0,
        };
    }

    outbound::send(Content::AI { pins: state }, timer.stop()).await;
}
