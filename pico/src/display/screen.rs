use embedded_hal_1::digital::OutputPin;
use embedded_hal_async::spi::SpiDevice;
use heapless::{String, Vec};
use ssd1306::size::DisplaySizeAsync;
use {defmt_rtt as _, panic_probe as _};

use crate::display::device::Device;
use crate::measurements::Measurements;

#[derive(PartialEq)]
pub enum Value {
    None,
    OffOn(u8),
    OnOff(u8),
    Number(u8),
}

struct Line {
    text: String<16>,
    value: Value,
}

struct Page {
    lines: Vec<Line, 5>,
}

pub struct Screen<SPI, DC, SIZE: DisplaySizeAsync> {
    pages: Vec<Page, 3>,
    device: Device<SPI, DC, SIZE>,
}

impl<SPI, DC, SIZE> Screen<SPI, DC, SIZE>
where
    SIZE: DisplaySizeAsync,
{
    pub fn new(device: Device<SPI, DC, SIZE>) -> Self
    where
        SIZE: DisplaySizeAsync,
    {
        let mut screen = Screen {
            pages: Vec::new(),
            device: device,
        };
        screen.push_page();
        screen
    }

    pub fn push_page(&mut self) {
        let page = Page { lines: Vec::new() };
        let _ = self.pages.push(page);
    }

    pub fn pop_page(&mut self) {
        self.pages.pop();
    }

    pub fn add_line(&mut self, text: &str, value: Value) {
        let Some(page) = self.pages.last_mut() else {
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

    pub async fn draw(&mut self, measurements: &Measurements)
    where
        SPI: SpiDevice,
        DC: OutputPin,
        SIZE: DisplaySizeAsync,
    {
        let device = &mut self.device;
        device.clear();

        let Some(page) = self.pages.last() else {
            return;
        };

        for (row, line) in (0..).zip(page.lines.iter()) {
            device.draw_text(row, line.text.as_str());

            match line.value {
                Value::None => {}
                Value::OffOn(pin) => device.draw_off_on(row, measurements[pin as usize] != 0),
                Value::OnOff(pin) => device.draw_off_on(row, measurements[pin as usize] == 0),
                Value::Number(pin) => device.draw_number(row, measurements[pin as usize]),
            }
        }
        device.refresh().await;
    }
}
