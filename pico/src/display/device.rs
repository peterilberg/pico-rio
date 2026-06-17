use display_interface::DisplayError;
use display_interface_spi::SPIInterface;
use embedded_graphics::mono_font::ascii::FONT_6X10;
use embedded_graphics::mono_font::{MonoTextStyle, MonoTextStyleBuilder};
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::{PrimitiveStyle, PrimitiveStyleBuilder};
use embedded_graphics::primitives::{Rectangle, StyledDrawable};
use embedded_graphics::text::Text;
use embedded_hal_1::digital::OutputPin;
use embedded_hal_async::spi::SpiDevice;
use heapless::format;
use ssd1306::{Ssd1306Async, rotation::DisplayRotation, size::DisplaySizeAsync};
use ssd1306::{mode::BufferedGraphicsModeAsync, prelude::*};
use {defmt_rtt as _, panic_probe as _};

static STYLE_TEXT: MonoTextStyle<'static, BinaryColor> = MonoTextStyleBuilder::new()
    .font(&FONT_6X10)
    .text_color(BinaryColor::On)
    .build();

static STYLE_ON: MonoTextStyle<'static, BinaryColor> = MonoTextStyleBuilder::new()
    .font(&FONT_6X10)
    .background_color(BinaryColor::On)
    .text_color(BinaryColor::Off)
    .build();

static STYLE_OFF: MonoTextStyle<'static, BinaryColor> = MonoTextStyleBuilder::new()
    .font(&FONT_6X10)
    .text_color(BinaryColor::On)
    .build();

static STYLE_EMPTY: PrimitiveStyle<BinaryColor> = PrimitiveStyleBuilder::new()
    .stroke_color(BinaryColor::On)
    .fill_color(BinaryColor::Off)
    .stroke_width(1)
    .build();

static STYLE_FULL: PrimitiveStyle<BinaryColor> = PrimitiveStyleBuilder::new()
    .stroke_color(BinaryColor::On)
    .fill_color(BinaryColor::On)
    .stroke_width(1)
    .build();

const LEFT_COLUMN: i32 = 0;
const RIGHT_COLUMN: i32 = 100;
const ROW_HEIGHT: i32 = 12;
const ON_OFF_OFFSET: i32 = 4;
const ON_OFF_WIDTH: u32 = 23;
const ON_OFF_HEIGHT: u32 = 11;
const ON_TEXT_OFFSET: i32 = 6;
const OFF_TEXT_OFFSET: i32 = 3;

pub struct Device<SPI, DC, SIZE>(
    Ssd1306Async<SPIInterface<SPI, DC>, SIZE, BufferedGraphicsModeAsync<SIZE>>,
)
where
    SIZE: DisplaySizeAsync;

impl<SPI, DC, SIZE> Device<SPI, DC, SIZE>
where
    SPI: SpiDevice,
    DC: OutputPin,
    SIZE: DisplaySizeAsync,
{
    pub async fn build(spi: SPIInterface<SPI, DC>, size: SIZE) -> Result<Self, DisplayError> {
        let mut target =
            Ssd1306Async::new(spi, size, DisplayRotation::Rotate0).into_buffered_graphics_mode();
        target.init().await?;
        Ok(Device(target))
    }

    pub fn clear(&mut self) {
        let Device(target) = self;
        let _ = target.clear(BinaryColor::Off);
    }

    pub async fn refresh(&mut self) {
        let Device(target) = self;
        let _ = target.flush().await;
    }

    pub fn draw_off_on(&mut self, row: i32, value: bool) {
        let Device(target) = self;
        let (text, offset, text_style, rect_style) = if value {
            ("on", ON_TEXT_OFFSET, STYLE_ON, &STYLE_FULL)
        } else {
            ("off", OFF_TEXT_OFFSET, STYLE_OFF, &STYLE_EMPTY)
        };

        let top_left = Point::new(RIGHT_COLUMN, ROW_HEIGHT * row + ON_OFF_OFFSET);
        let size = Size::new(ON_OFF_WIDTH, ON_OFF_HEIGHT);
        let _ = Rectangle::new(top_left, size).draw_styled(rect_style, target);

        let row = row + 1;
        let position = Point::new(RIGHT_COLUMN + offset, ROW_HEIGHT * row);
        let _ = Text::new(text, position, text_style).draw(target);
    }

    pub fn draw_number(&mut self, row: i32, number: u8) {
        let Device(target) = self;
        let row = row + 1;

        let Ok(text) = format!(4; "{:3}%", number) else {
            return;
        };

        let position = Point::new(RIGHT_COLUMN, ROW_HEIGHT * row);
        let _ = Text::new(text.as_str(), position, STYLE_TEXT).draw(target);
    }

    pub fn draw_text(&mut self, row: i32, text: &str) {
        let Device(target) = self;
        let row = row + 1;

        let position = Point::new(LEFT_COLUMN, ROW_HEIGHT * row);
        let _ = Text::new(text, position, STYLE_TEXT).draw(target);
    }
}
