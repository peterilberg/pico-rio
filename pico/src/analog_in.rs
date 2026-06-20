use embassy_rp::Peri;
use embassy_rp::adc::{Adc, Async, Channel, Config, InterruptHandler};
use embassy_rp::bind_interrupts;
use embassy_rp::peripherals::ADC;
use embassy_time::Duration;
use messages::{Content, Pins};
use {defmt_rtt as _, panic_probe as _};

use crate::measurements;
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
pub async fn task(interval: Duration, adc: Peri<'static, ADC>, mut pins: Pins<Channel<'static>>) {
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

    let mut state = Pins::<u8>::new();
    for (pin, input) in pins {
        let _ = state.push((
            *pin,
            match adc.read(input).await {
                Ok(value) => (value as f32 / 40.96) as u8,
                Err(_) => 0,
            },
        ));
    }

    measurements::set_analog(&state).await;
    outbound::send(Content::AI { pins: state }, timer.stop()).await;
}
