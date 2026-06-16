use core::fmt::Write;

use display_interface_spi::SPIInterface;
use embassy_rp::gpio::Output;
use embassy_rp::peripherals::SPI0;
use embassy_rp::spi::{Async, Blocking, Spi};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embassy_time::Delay;
use embedded_graphics::mono_font::MonoTextStyleBuilder;
use embedded_graphics::mono_font::ascii::FONT_6X10;
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::{PrimitiveStyleBuilder, Rectangle, StyledDrawable};
use embedded_graphics::text::Text;
use embedded_hal_bus::spi::ExclusiveDevice;
use heapless::{String, format};
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
    embassy_time::Timer::after_secs(1).await;

    let device = ExclusiveDevice::new(config.spi, config.cs, Delay);
    let display = SPIInterface::new(device, config.d_c);

    let mut delay = embassy_time::Delay;
    let mut display = ssd1306::Ssd1306Async::new(
        display,
        ssd1306::size::DisplaySize128x64,
        ssd1306::rotation::DisplayRotation::Rotate0,
    )
    .into_buffered_graphics_mode();
    display.init().await;
    display.clear(embedded_graphics::pixelcolor::BinaryColor::Off);
    let style = MonoTextStyleBuilder::new()
        .font(&FONT_6X10)
        .text_color(embedded_graphics::pixelcolor::BinaryColor::On)
        .build();
    let off_style = MonoTextStyleBuilder::new()
        .font(&FONT_6X10)
        .text_color(embedded_graphics::pixelcolor::BinaryColor::On)
        .build();
    let on_style = MonoTextStyleBuilder::new()
        .font(&FONT_6X10)
        .background_color(embedded_graphics::pixelcolor::BinaryColor::On)
        .text_color(embedded_graphics::pixelcolor::BinaryColor::Off)
        .build();
    let empty_style = PrimitiveStyleBuilder::new()
        .stroke_color(embedded_graphics::pixelcolor::BinaryColor::On)
        .fill_color(embedded_graphics::pixelcolor::BinaryColor::Off)
        .stroke_width(1)
        .build();
    let rect_style = PrimitiveStyleBuilder::new()
        .stroke_color(embedded_graphics::pixelcolor::BinaryColor::On)
        .fill_color(embedded_graphics::pixelcolor::BinaryColor::On)
        .stroke_width(1)
        .build();

    Text::new("Water tank", Point::new(0, 12), style)
        .draw(&mut display)
        .unwrap();
    for i in 1..=2 {
        let s = format!(64; "Something {}", i).unwrap();
        Text::new(s.as_str(), Point::new(0, 12 * (i + 1)), style)
            .draw(&mut display)
            .unwrap();
        let s = format!(64; "{}%", i*100).unwrap();
        Text::new(s.as_str(), Point::new(100, 12 * (i + 1)), style)
            .draw(&mut display)
            .unwrap();
    }
    let s = format!(64; "Valve {}", 3).unwrap();
    Text::new(s.as_str(), Point::new(0, 12 * (3 + 1)), style)
        .draw(&mut display)
        .unwrap();
    Rectangle::new(
        Point::new(100, 12 * 3 + 4),
        Size {
            width: 23,
            height: 11,
        },
    )
    .draw_styled(&empty_style, &mut display)
    .unwrap();
    Text::new("off", Point::new(103, 12 * (3 + 1)), off_style)
        .draw(&mut display)
        .unwrap();

    let s = format!(64; "Valve {}", 4).unwrap();
    Text::new(s.as_str(), Point::new(0, 12 * (4 + 1)), style)
        .draw(&mut display)
        .unwrap();
    Rectangle::new(
        Point::new(100, 12 * 4 + 4),
        Size {
            width: 23,
            height: 11,
        },
    )
    .draw_styled(&rect_style, &mut display)
    .unwrap();
    Text::new("on", Point::new(106, 12 * (4 + 1)), on_style)
        .draw(&mut display)
        .unwrap();

    display.flush().await;

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
    pub spi: Spi<'static, SPI0, Async>,
    /// The latch to update the register, called "RCLK" in the datasheet.
    pub reset: Output<'static>,
    /// The clock to shift a bit, called "SRCLK" in the datasheet.
    pub d_c: Output<'static>,
    /// The bits to be shifted, called "SER" in the datasheet.
    pub cs: Output<'static>,
}
