use core::fmt::Write;

use display_interface_spi::SPIInterface;
use embassy_rp::gpio::Output;
use embassy_rp::peripherals::SPI0;
use embassy_rp::spi::{Async, Blocking, Spi};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embedded_hal_bus::spi::ExclusiveDevice;
use messages::NUM_PINS;
use ssd1306::{Ssd1306, prelude::*};
use {defmt_rtt as _, panic_probe as _};

use crate::measurements::Measurements;
use crate::network;

enum Message {
    Select { pin: u8 },
    Measurements { measurements: Measurements },
}

static INBOX: Channel<CriticalSectionRawMutex, Message, 16> = Channel::new();

pub async fn notify(measurements: &Measurements) {
    let measurements = *measurements;
    INBOX.send(Message::Measurements { measurements }).await;
}

pub async fn select(pin: u8) {
    INBOX.send(Message::Select { pin }).await;
}

#[embassy_executor::task]
pub async fn task(mut config: Config) {
    network::wait_for_network().await;
    embassy_time::Timer::after_secs(5).await;

    log::info!("0");
    let device = ExclusiveDevice::new_no_delay(config.spi, config.cs);
    let display = SPIInterface::new(device, config.d_c);
    log::info!("0");

    let mut delay = embassy_time::Delay;
    let mut terminal = ssd1306::Ssd1306Async::new(
        display,
        ssd1306::size::DisplaySize128x64,
        ssd1306::rotation::DisplayRotation::Rotate0,
    )
    .into_terminal_mode();
    terminal.reset(&mut config.reset, &mut delay);
    log::info!("{:?}", terminal.init().await);
    log::info!("0");
    terminal.clear().await;
    //log::info!("{:?}", terminal.clear().await);
    //log::info!("{:?}", terminal.set_display_on(true).await);
    log::info!("{:?}", terminal.write_str("What's up?").await);
    //let graphics = terminal.into_buffered_graphics_mode();
    //graphics.flush().await;

    //
    // let mut selection = None;
    // let mut measurements = [0_u8; NUM_PINS];

    loop {
        embassy_time::Timer::after_secs(5).await;
        log::info!("loop");
        /*
                match INBOX.receive().await {
                    Message::Select { pin } => {
                        selection = if pin as usize >= NUM_PINS {
                            None
                        } else {
                            Some(pin)
                        };
                    }
                    Message::Measurements {
                        measurements: new_values,
                    } => {
                        measurements = new_values;
                    }
                }
        */
    }
}

/// The bar graph is powered by an 8-bit shift register of type 74HC595.
pub struct Config {
    pub spi: Spi<'static, SPI0, Async>,
    /// The latch to update the register, called "RCLK" in the datasheet.
    pub reset: Output<'static>,
    /// The clock to shift a bit, called "SRCLK" in the datasheet.
    pub d_c: Output<'static>,
    /// The bits to be shifted, called "SER" in the datasheet.
    pub cs: Output<'static>,
}
