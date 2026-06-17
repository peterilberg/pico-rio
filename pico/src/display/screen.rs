use display_interface::DisplayError;
use heapless::String;
use {defmt_rtt as _, panic_probe as _};

use super::device;

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

pub struct Screen<SPI, DC> {
    stack: Stack,
    display: device::Display<SPI, DC>,
}

impl<SPI, DC> Screen<SPI, DC> {
    pub fn build(display: device::Display<SPI, DC>) -> Result<Self, DisplayError> {
        let mut screen = Screen {
            stack: Stack::new(),
            display: display,
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

    pub async fn draw(&mut self)
    where
        SPI: embedded_hal_async::spi::SpiDevice,
        DC: embedded_hal_1::digital::OutputPin,
    {
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
