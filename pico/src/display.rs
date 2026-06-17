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

    let mut screen = screen::Screen::build(display).await.unwrap();

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

mod screen {
    use display_interface::DisplayError;
    use display_interface_spi::SPIInterface;
    use embassy_rp::gpio::{Level, Output};
    use embassy_rp::peripherals::SPI0;
    use embassy_rp::spi::{Async, Spi};
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
    use ssd1306::prelude::*;
    use {defmt_rtt as _, panic_probe as _};

    #[derive(Debug, PartialEq)]
    pub enum Value {
        None,
        OffOn(u8),
        OnOff(u8),
        Number(u8),
    }

    #[derive(Debug)]
    struct Line {
        text: heapless::String<16>,
        value: Value,
    }

    struct Page {
        lines: heapless::Vec<Line, 5>,
    }

    type Stack = heapless::Vec<Page, 3>;

    pub struct Screen<'display> {
        stack: Stack,
        display: super::display::Display<'display>,
    }

    impl<'screen> Screen<'screen> {
        pub async fn build(
            spi: SPIInterface<
                ExclusiveDevice<
                    embassy_rp::spi::Spi<'screen, SPI0, embassy_rp::spi::Async>,
                    Output<'screen>,
                    Delay,
                >,
                Output<'screen>,
            >,
        ) -> Result<Self, DisplayError> {
            let mut display = ssd1306::Ssd1306Async::new(
                spi,
                ssd1306::size::DisplaySize128x64,
                ssd1306::rotation::DisplayRotation::Rotate0,
            )
            .into_buffered_graphics_mode();
            display.init().await?;

            let mut screen = Screen {
                stack: Stack::new(),
                display: super::display::Display::new(display),
            };
            screen.push_page();
            Ok(screen)
        }

        pub fn push_page(&mut self) {
            let page = Page {
                lines: heapless::Vec::new(),
            };
            let _ = self.stack.push(page);
        }

        pub fn pop_page(&mut self) {
            self.stack.pop();
        }

        pub fn add_line(&mut self, text: &str, value: Value) {
            let Some(page) = self.stack.last_mut() else {
                return;
            };
            let Ok(text) = String::try_from(text) else {
                return;
            };

            let _ = page.lines.push(Line {
                text: text,
                value: value,
            });
        }

        pub async fn draw(&mut self) {
            let display = &mut self.display;

            display.clear();
            let Some(screen) = self.stack.last() else {
                return;
            };

            for (row, line) in (0..).zip(screen.lines.iter()) {
                log::info!("{} {:?}", row, line);
                display.draw_text(row, line.text.as_str());

                match line.value {
                    Value::None => {}
                    Value::OffOn(pin) => display.draw_off_on(row, 0 != 0),
                    Value::OnOff(pin) => display.draw_off_on(row, 0 == 0),
                    Value::Number(pin) => display.draw_number(row, 42),
                }
            }
            display.refresh().await;
        }
    }
}

mod display {
    use display_interface::DisplayError;
    use display_interface_spi::SPIInterface;
    use embassy_rp::gpio::{Level, Output};
    use embassy_rp::peripherals::SPI0;
    use embassy_rp::spi::{Async, Spi};
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
    use ssd1306::prelude::*;
    use {defmt_rtt as _, panic_probe as _};

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

    pub struct Display<'display>(
        ssd1306::Ssd1306Async<
            SPIInterface<
                ExclusiveDevice<Spi<'display, SPI0, Async>, Output<'display>, Delay>,
                Output<'display>,
            >,
            DisplaySize128x64,
            ssd1306::mode::BufferedGraphicsModeAsync<DisplaySize128x64>,
        >,
    );

    impl<'display> Display<'display> {
        pub fn new(
            display: ssd1306::Ssd1306Async<
                SPIInterface<
                    ExclusiveDevice<Spi<'display, SPI0, Async>, Output<'display>, Delay>,
                    Output<'display>,
                >,
                DisplaySize128x64,
                ssd1306::mode::BufferedGraphicsModeAsync<DisplaySize128x64>,
            >,
        ) -> Self {
            Display(display)
        }

        pub fn clear(&mut self) {
            let Display(display) = self;
            let _ = display.clear(embedded_graphics::pixelcolor::BinaryColor::Off);
        }

        pub async fn refresh(&mut self) {
            let Display(display) = self;
            let _ = display.flush().await;
        }

        pub fn draw_off_on(&mut self, row: i32, value: bool) {
            let Display(display) = self;
            let (text, offset, text_style, rect_style) = if value {
                ("on", 6, STYLE_ON, &STYLE_FULL)
            } else {
                ("off", 3, STYLE_OFF, &STYLE_EMPTY)
            };

            Rectangle::new(
                Point::new(100, 12 * row + 4),
                Size {
                    width: 23,
                    height: 11,
                },
            )
            .draw_styled(rect_style, display)
            .unwrap();
            Text::new(text, Point::new(100 + offset, 12 * (row + 1)), text_style)
                .draw(display)
                .unwrap();
        }

        pub fn draw_number(&mut self, row: i32, number: u8) {
            let Display(display) = self;
            let s = format!(4; "{:3}%", number).unwrap();
            Text::new(s.as_str(), Point::new(100, 12 * (row + 1)), STYLE_TEXT)
                .draw(display)
                .unwrap();
        }

        pub fn draw_text(&mut self, row: i32, text: &str) {
            let Display(display) = self;
            Text::new(text, Point::new(0, 12 * (row + 1)), STYLE_TEXT)
                .draw(display)
                .unwrap();
        }
    }
}
