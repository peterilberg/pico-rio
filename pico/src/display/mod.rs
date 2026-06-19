use display_interface_spi::SPIInterface;
use embassy_rp::gpio::{Level, Output};
use embassy_rp::peripherals::SPI0;
use embassy_rp::spi::{Async, Spi};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embassy_time::Delay;
use embedded_hal_bus::spi::ExclusiveDevice;
use heapless::String;
use ssd1306::size::DisplaySize128x64;
use {defmt_rtt as _, panic_probe as _};

use messages::Value;

use crate::display::device::Device;
use crate::display::screen::Screen;
use crate::measurements::Measurements;
use crate::network;

mod device;
mod screen;

enum Message {
    Measurements { measurements: Measurements },

    Clear,
    AddLine { label: String<16>, value: Value },
    Refresh,

    AddPage,
    RemovePage,
}

static INBOX: Channel<CriticalSectionRawMutex, Message, 32> = Channel::new();

pub async fn notify(measurements: &Measurements) {
    let measurements = *measurements;
    INBOX.send(Message::Measurements { measurements }).await;
}

pub async fn clear() {
    INBOX.send(Message::Clear).await;
}

pub async fn add_line(label: String<16>, value: Value) {
    INBOX.send(Message::AddLine { label, value }).await;
}

#[allow(dead_code)]
pub async fn add_text(label: &str, value: Value) {
    if let Ok(label) = String::try_from(label) {
        INBOX.send(Message::AddLine { label, value }).await;
    }
}

pub async fn refresh() {
    INBOX.send(Message::Refresh).await;
}

pub async fn add_page() {
    INBOX.send(Message::AddPage).await;
}

pub async fn remove_page() {
    INBOX.send(Message::RemovePage).await;
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
    let mut measurements = Measurements::default();

    loop {
        match INBOX.receive().await {
            Message::Measurements {
                measurements: new_values,
            } => {
                measurements = new_values;
                screen.draw(&measurements).await;
            }
            Message::Clear => {
                screen.clear();
            }
            Message::AddLine { label, value } => {
                screen.add_line(label, value);
            }
            Message::Refresh => {
                screen.draw(&measurements).await;
            }
            Message::AddPage => {
                screen.push_page();
            }
            Message::RemovePage => {
                screen.pop_page();
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
