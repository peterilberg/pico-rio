use embassy_rp::gpio::{Level, Output};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use messages::NUM_PINS;
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
pub async fn task(mut control: Control) {
    network::wait_for_network().await;

    let mut selection = None;
    let mut measurements = [0_u8; NUM_PINS];

    shift_value(&mut control, get_value(&selection, &measurements)).await;

    loop {
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
        shift_value(&mut control, get_value(&selection, &measurements)).await;
    }
}

fn get_value(selection: &Option<u8>, measurements: &[u8]) -> u8 {
    match selection {
        None => 0,
        Some(i) => measurements[*i as usize],
    }
}

/// The bar graph is powered by an 8-bit shift register of type 74HC595.
pub struct Control {
    /// The latch to update the register, called "RCLK" in the datasheet.
    pub latch: Output<'static>,
    /// The clock to shift a bit, called "SRCLK" in the datasheet.
    pub clock: Output<'static>,
    /// The bits to be shifted, called "SER" in the datasheet.
    pub data: Output<'static>,
}

async fn shift_value(control: &mut Control, value: u8) {
    // Input ranges from 0 - 100, output is a bar graph with 8 LEDs.
    // Scale input down to 0 - 8 and convert to bit pattern:
    //      0 -> 0b_0000_0000
    //      1 -> 0b_0000_0001
    //      2 -> 0b_0000_0011
    //      3 -> 0b_0000_0111
    //      ...
    //      8 -> 0b_1111_1111
    let mut bits = (1_u32 << (value / 12)) - 1;

    control.latch.set_low();

    // Shift bits from lowest to highest.
    for _ in 0..8 {
        static LEVELS: [Level; 2] = [Level::Low, Level::High];
        control.data.set_level(LEVELS[(bits & 1) as usize]);
        bits >>= 1;

        control.clock.set_high();
        embassy_time::Timer::after_micros(1).await;
        control.clock.set_low();
    }

    control.latch.set_high();
}
