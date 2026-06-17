use display_interface_spi::SPIInterface;
use embassy_rp::gpio::{Level, Output};
use embassy_rp::peripherals::SPI0;
use embassy_rp::spi::{Async, Spi};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embassy_time::Delay;
use embedded_hal_bus::spi::ExclusiveDevice;
use {defmt_rtt as _, panic_probe as _};

use crate::measurements::Measurements;
use crate::network;

mod device;
mod screen;

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
    embassy_time::Timer::after_secs(1).await;

    config.reset.set_level(Level::High);
    let device = ExclusiveDevice::new(config.spi0, config.cs, Delay);
    let display = SPIInterface::new(device, config.d_c);

    let mut graphics = device::Display::build(display).await.unwrap();
    let mut screen = screen::Screen::build(graphics).unwrap();

    // TODO send to self
    // process messages
    // Screen is five lines of fixed size string<16> plus type: none, offon, onoff, number, idx
    // stack of 3 screens
    // redraw topmost screen

    screen.add_line("Water tank", screen::Value::None);
    screen.add_line("Something 1", screen::Value::Number(42));
    screen.add_line("Something 2", screen::Value::Number(100));
    screen.add_line("Valve 3", screen::Value::OffOn(100));
    screen.add_line("Valve 4", screen::Value::OnOff(100));
    screen.draw().await;

    // let mut selection = None;
    // let mut measurements = [0_u8; NUM_PINS];

    loop {
        embassy_time::Timer::after_secs(1).await;
        // log::info!("loop");
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
    /// The SPI0 interface.
    /// Async embassy tasks cannot be accept a template argument.
    pub spi0: Spi<'static, SPI0, Async>,
    /// The latch to update the register, called "RCLK" in the datasheet.
    pub reset: Output<'static>,
    /// The clock to shift a bit, called "SRCLK" in the datasheet.
    pub d_c: Output<'static>,
    /// The bits to be shifted, called "SER" in the datasheet.
    pub cs: Output<'static>,
}
