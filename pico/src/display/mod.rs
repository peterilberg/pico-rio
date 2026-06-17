use display_interface_spi::SPIInterface;
use embassy_rp::gpio::{Level, Output};
use embassy_rp::peripherals::SPI0;
use embassy_rp::spi::{Async, Spi};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embassy_time::Delay;
use embedded_hal_bus::spi::ExclusiveDevice;
use ssd1306::size::DisplaySize128x64;
use {defmt_rtt as _, panic_probe as _};

use crate::display::device::Device;
use crate::display::screen::Screen;
use crate::measurements::Measurements;
use crate::network;
use messages::NUM_PINS;

mod device;
mod screen;

enum Message {
    Measurements { measurements: Measurements },
}

static INBOX: Channel<CriticalSectionRawMutex, Message, 16> = Channel::new();

pub async fn notify(measurements: &Measurements) {
    let measurements = *measurements;
    INBOX.send(Message::Measurements { measurements }).await;
}

#[embassy_executor::task]
pub async fn task(mut config: Config) {
    network::wait_for_network().await;

    config.reset.set_level(Level::High);
    let device = ExclusiveDevice::new(config.spi0, config.cs, Delay);
    let display = SPIInterface::new(device, config.d_c);

    let device = match Device::build(display, DisplaySize128x64).await {
        Ok(device) => device,
        Err(error) => {
            log::info!("display: device initialization failed {:?}", error);
            return;
        }
    };

    let mut screen = Screen::new(device);

    let mut measurements = [0_u8; NUM_PINS];

    // TODO send to self
    screen.add_line("Water tank", screen::Value::None);
    screen.add_line("Pump", screen::Value::Number(26));
    screen.add_line("Fill level", screen::Value::Number(27));
    screen.add_line("Fire", screen::Value::OnOff(19));
    screen.draw(&measurements).await;

    loop {
        match INBOX.receive().await {
            Message::Measurements {
                measurements: new_values,
            } => {
                measurements = new_values;
                screen.draw(&measurements).await;
            }
        }
    }
}

/// The bar graph is powered by an 8-bit shift register of type 74HC595.
pub struct Config {
    /// The SPI0 interface. Fixed to SPI0 because async
    /// embassy tasks cannot accept template arguments.
    pub spi0: Spi<'static, SPI0, Async>,
    /// The latch to update the register, called "RCLK" in the datasheet.
    pub reset: Output<'static>,
    /// The clock to shift a bit, called "SRCLK" in the datasheet.
    pub d_c: Output<'static>,
    /// The bits to be shifted, called "SER" in the datasheet.
    pub cs: Output<'static>,
}
