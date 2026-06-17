use display_interface::DisplayError;
use display_interface_spi::SPIInterface;
use embassy_rp::gpio::Output;
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
use heapless::format;
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

pub struct Display<SPI, DC>(
    ssd1306::Ssd1306Async<
        SPIInterface<SPI, DC>,
        DisplaySize128x64,
        ssd1306::mode::BufferedGraphicsModeAsync<DisplaySize128x64>,
    >,
);

impl<SPI, DC> Display<SPI, DC>
where
    SPI: embedded_hal_async::spi::SpiDevice,
    DC: embedded_hal_1::digital::OutputPin,
{
    pub async fn build(spi: SPIInterface<SPI, DC>) -> Result<Self, DisplayError> {
        let mut display = ssd1306::Ssd1306Async::new(
            spi,
            ssd1306::size::DisplaySize128x64,
            ssd1306::rotation::DisplayRotation::Rotate0,
        )
        .into_buffered_graphics_mode();
        display.init().await?;
        Ok(Display(display))
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
