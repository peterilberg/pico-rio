use embassy_futures::select::{Either, select};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embassy_time::Duration;
use heapless::String;
use messages::{BangBang, Content, Diagnostics, Mode, Value};
use {defmt_rtt as _, panic_probe as _};

use crate::analog_out;
use crate::display;
use crate::measurements::Measurements;
use crate::network;
use crate::outbound;
use crate::timer::Timer;
use crate::watchdog;

enum Message {
    Start,
    Stop,

    SetInput { pin: u8 },
    SetOutput { pin: u8 },

    SetLowerLimit { value: u8 },
    SetUpperLimit { value: u8 },

    Show,
    Hide,

    Measurements { measurements: Measurements },
}

static INBOX: Channel<CriticalSectionRawMutex, Message, 16> = Channel::new();

pub async fn notify(measurements: &Measurements) {
    let measurements = *measurements;
    INBOX.send(Message::Measurements { measurements }).await;
}

pub async fn start() {
    INBOX.send(Message::Start).await;
}

pub async fn stop() {
    INBOX.send(Message::Stop).await;
}

pub async fn set_input(pin: u8) {
    INBOX.send(Message::SetInput { pin }).await;
}

pub async fn set_output(pin: u8) {
    INBOX.send(Message::SetOutput { pin }).await;
}

pub async fn set_lower_limit(value: u8) {
    INBOX.send(Message::SetLowerLimit { value }).await;
}

pub async fn set_upper_limit(value: u8) {
    INBOX.send(Message::SetUpperLimit { value }).await;
}

pub async fn show() {
    INBOX.send(Message::Show).await;
}

pub async fn hide() {
    INBOX.send(Message::Hide).await;
}

#[embassy_executor::task]
pub async fn task(interval: Duration) {
    network::wait_for_network().await;

    let mut settings = BangBang {
        mode: Mode::Off,
        input: 0,
        output: 0,
        lower_limit: 0,
        upper_limit: 0,
    };
    let mut measurements = Measurements::default();
    let mut visible = false;

    let mut timer = Timer::new(interval);
    loop {
        match select(timer.wait(), INBOX.receive()).await {
            Either::First(()) => {
                timer.start();
                run(&mut settings, &measurements).await;
                send_state(&settings, timer.stop()).await;
            }
            Either::Second(Message::Start) => {
                settings.mode = Mode::Running;
                update_display(&settings, visible).await;
            }
            Either::Second(Message::Stop) => {
                analog_out::set_pin(settings.output, 0).await;
                settings.mode = Mode::Off;
                update_display(&settings, visible).await;
            }
            Either::Second(Message::SetInput { pin }) => {
                settings.input = pin;
            }
            Either::Second(Message::SetOutput { pin }) => {
                settings.output = pin;
            }
            Either::Second(Message::SetLowerLimit { value }) => {
                settings.lower_limit = value;
                update_display(&settings, visible).await;
            }
            Either::Second(Message::SetUpperLimit { value }) => {
                settings.upper_limit = value;
                update_display(&settings, visible).await;
            }
            Either::Second(Message::Measurements {
                measurements: new_values,
            }) => {
                measurements = new_values;
            }
            Either::Second(Message::Show) => {
                display::add_page().await;
                visible = true;
                update_display(&settings, visible).await;
            }
            Either::Second(Message::Hide) => {
                display::remove_page().await;
                visible = false;
            }
        }

        watchdog::notify();
    }
}

async fn send_state(settings: &BangBang, diagnostics: Diagnostics) {
    outbound::send(
        Content::BangBang {
            settings: settings.clone(),
        },
        diagnostics,
    )
    .await;
}

async fn run(settings: &mut BangBang, measurements: &Measurements) {
    match settings.mode {
        Mode::Off => {}
        Mode::Running => {
            let value = measurements[settings.input as usize];
            if value < settings.upper_limit {
                let value = (settings.upper_limit - value) * 2;
                if value != measurements[settings.output as usize] {
                    analog_out::set_pin(settings.output, value).await;
                }
            } else {
                analog_out::set_pin(settings.output, 0).await;
                settings.mode = Mode::Waiting;
            }
        }
        Mode::Waiting => {
            let value = measurements[settings.input as usize];
            if value < settings.lower_limit {
                settings.mode = Mode::Running;
            }
        }
    }
}

async fn update_display(settings: &BangBang, visible: bool) {
    if !visible {
        return;
    }

    let enabled = settings.mode != Mode::Off;

    display::clear().await;
    display::add_line(label("Controller"), Value::Boolean(enabled)).await;
    display::add_line(label("Output"), Value::Analog(settings.output)).await;
    display::add_line(label("Upper limit"), Value::Number(settings.upper_limit)).await;
    display::add_line(label("Input"), Value::Analog(settings.input)).await;
    display::add_line(label("Lower limit"), Value::Number(settings.lower_limit)).await;
    display::refresh().await;
}

fn label(label: &str) -> String<16> {
    match String::try_from(label) {
        Ok(string) => string,
        Err(_) => String::new(),
    }
}
