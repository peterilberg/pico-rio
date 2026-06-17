use core::fmt::Write;

use display_interface_spi::SPIInterface;
use embassy_rp::gpio::{Level, Output};
use embassy_rp::peripherals::SPI0;
use embassy_rp::spi::{Async, Blocking, Spi};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embassy_time::Delay;
use embedded_graphics::mono_font::ascii::FONT_6X10;
use embedded_graphics::mono_font::{MonoTextStyle, MonoTextStyleBuilder};
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::{
    PrimitiveStyle, PrimitiveStyleBuilder, Rectangle, StyledDrawable,
};
use embedded_graphics::text::Text;
use embedded_hal_bus::spi::ExclusiveDevice;
use heapless::{String, format};
use messages::NUM_PINS;
use ssd1306::{Ssd1306, prelude::*};
use {defmt_rtt as _, panic_probe as _};

use crate::measurements::Measurements;
use crate::{display, network};

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

static STYLE_TEXT: MonoTextStyle<'static, BinaryColor> = MonoTextStyleBuilder::new()
    .font(&FONT_6X10)
    .text_color(embedded_graphics::pixelcolor::BinaryColor::On)
    .build();
static STYLE_ON: MonoTextStyle<'static, BinaryColor> = MonoTextStyleBuilder::new()
    .font(&FONT_6X10)
    .background_color(embedded_graphics::pixelcolor::BinaryColor::On)
    .text_color(embedded_graphics::pixelcolor::BinaryColor::Off)
    .build();
static STYLE_OFF: MonoTextStyle<'static, BinaryColor> = MonoTextStyleBuilder::new()
    .font(&FONT_6X10)
    .text_color(embedded_graphics::pixelcolor::BinaryColor::On)
    .build();

static STYLE_EMPTY: PrimitiveStyle<BinaryColor> = PrimitiveStyleBuilder::new()
    .stroke_color(embedded_graphics::pixelcolor::BinaryColor::On)
    .fill_color(embedded_graphics::pixelcolor::BinaryColor::Off)
    .stroke_width(1)
    .build();

static STYLE_FULL: PrimitiveStyle<BinaryColor> = PrimitiveStyleBuilder::new()
    .stroke_color(embedded_graphics::pixelcolor::BinaryColor::On)
    .fill_color(embedded_graphics::pixelcolor::BinaryColor::On)
    .stroke_width(1)
    .build();

#[derive(PartialEq)]
enum Value {
    None,
    OffOn(u8),
    OnOff(u8),
    Number(u8),
}

struct Line {
    text: heapless::String<16>,
    value: Value,
}

struct Screen {
    lines: heapless::Vec<Line, 5>,
}

type Stack = heapless::Vec<Screen, 3>;

static STACK: Stack = Stack::new();

fn add_line<'stack>(stack: &'stack mut Stack, line: Line) {
    if let Some(screen) = stack.last_mut() {
        let _ = screen.lines.push(line);
    };
}

fn push_screen<'stack>(stack: &'stack mut Stack) {
    let screen = Screen {
        lines: heapless::Vec::new(),
    };
    let _ = stack.push(screen);
}

fn pop_screen<'stack>(stack: &'stack mut Stack) {
    stack.pop();
}

type Display<'display> = ssd1306::Ssd1306Async<
    SPIInterface<
        ExclusiveDevice<Spi<'display, SPI0, Async>, Output<'display>, Delay>,
        Output<'display>,
    >,
    DisplaySize128x64,
    ssd1306::mode::BufferedGraphicsModeAsync<DisplaySize128x64>,
>;

fn draw_off_on(display: &mut Display, row: i32, value: bool) {
    let (text, offset, text_style, rect_style) = if value {
        ("on", 6, STYLE_ON, &STYLE_FULL)
    } else {
        ("off", 3, STYLE_OFF, &STYLE_EMPTY)
    };

    Rectangle::new(
        Point::new(100, 12 * (row - 1) + 4),
        Size {
            width: 23,
            height: 11,
        },
    )
    .draw_styled(rect_style, display)
    .unwrap();
    Text::new(text, Point::new(100 + offset, 12 * row), text_style)
        .draw(display)
        .unwrap();
}

fn draw_number(display: &mut Display, row: i32, number: u8) {
    let s = format!(4; "{:3}%", number).unwrap();
    Text::new(s.as_str(), Point::new(100, 12 * row), STYLE_TEXT)
        .draw(display)
        .unwrap();
}

fn draw_text(display: &mut Display, row: i32, text: &str) {
    Text::new(text, Point::new(0, 12 * row), STYLE_TEXT)
        .draw(display)
        .unwrap();
}

async fn draw_screen<'stack>(display: &mut Display<'_>, stack: &'stack mut Stack) {
    display.clear(embedded_graphics::pixelcolor::BinaryColor::Off);
    let Some(screen) = stack.last() else {
        return;
    };

    for (row, line) in (1..).zip(screen.lines.iter()) {
        draw_text(display, row, line.text.as_str());

        match line.value {
            Value::None => {}
            Value::OffOn(pin) => draw_off_on(display, row, 0 != 0),
            Value::OnOff(pin) => draw_off_on(display, row, 0 == 0),
            Value::Number(pin) => draw_number(display, row, 42),
        }
    }
    display.flush().await;
}

#[embassy_executor::task]
pub async fn task(mut config: Config) {
    network::wait_for_network().await;
    embassy_time::Timer::after_secs(1).await;

    config.reset.set_level(Level::High);
    let device = ExclusiveDevice::new(config.spi0, config.cs, Delay);
    let display = SPIInterface::new(device, config.d_c);

    let mut display = ssd1306::Ssd1306Async::new(
        display,
        ssd1306::size::DisplaySize128x64,
        ssd1306::rotation::DisplayRotation::Rotate0,
    )
    .into_buffered_graphics_mode();
    display.init().await;
    display.clear(embedded_graphics::pixelcolor::BinaryColor::Off);

    // TODO send to self
    // process messages
    // Screen is five lines of fixed size string<16> plus type: none, offon, onoff, number, idx
    // stack of 3 screens
    // redraw topmost screen

    let mut stack = Stack::new();
    push_screen(&mut stack);

    add_line(
        &mut stack,
        Line {
            text: String::try_from("Water tank").unwrap(),
            value: Value::None,
        },
    );

    add_line(
        &mut stack,
        Line {
            text: String::try_from("Something 1").unwrap(),
            value: Value::Number(42),
        },
    );

    add_line(
        &mut stack,
        Line {
            text: String::try_from("Something 2").unwrap(),
            value: Value::Number(100),
        },
    );

    add_line(
        &mut stack,
        Line {
            text: String::try_from("Valve 3").unwrap(),
            value: Value::OffOn(100),
        },
    );

    add_line(
        &mut stack,
        Line {
            text: String::try_from("Valve 4").unwrap(),
            value: Value::OnOff(100),
        },
    );

    draw_screen(&mut display, &mut stack).await;

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
