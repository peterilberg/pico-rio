use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use messages::Command;
use {defmt_rtt as _, panic_probe as _};

use crate::analog_out;
use crate::bang_bang;
use crate::digital_out;
use crate::display;
use crate::network;
use crate::sequencer;
use crate::watchdog;

enum Message {
    Command { command: Command },
}

static INBOX: Channel<CriticalSectionRawMutex, Message, 16> = Channel::new();

pub async fn dispatch(command: Command) {
    INBOX.send(Message::Command { command }).await;
}

#[embassy_executor::task]
pub async fn task() {
    network::wait_for_network().await;

    loop {
        let Message::Command { command } = INBOX.receive().await;
        match command {
            Command::Ping => {
                log::info!("dispatcher: inbound should handle ping");
            }
            Command::Restart => {
                watchdog::restart().await;
            }
            Command::Subscribe => {
                log::info!("dispatcher: inbound should handle subscribe");
            }
            Command::SetDO { pin, value } => {
                digital_out::set_pin(pin, value).await;
            }
            Command::SetAO { pin, value } => {
                analog_out::set_pin(pin, value).await;
            }
            Command::BangBangStart => {
                bang_bang::start().await;
            }
            Command::BangBangStop => {
                bang_bang::stop().await;
            }
            Command::BangBangInput { pin } => {
                bang_bang::set_input(pin).await;
            }
            Command::BangBangOutput { pin } => {
                bang_bang::set_output(pin).await;
            }
            Command::BangBangLowerLimit { value } => {
                bang_bang::set_lower_limit(value).await;
            }
            Command::BangBangUpperLimit { value } => {
                bang_bang::set_upper_limit(value).await;
            }
            Command::BangBangShow => {
                bang_bang::show().await;
            }
            Command::BangBangHide => {
                bang_bang::hide().await;
            }
            Command::RemovePage => {
                display::remove_page().await;
            }
            Command::AddPage => {
                display::add_page().await;
            }
            Command::ClearDisplay => {
                display::clear().await;
            }
            Command::AddLine { label, value } => {
                display::add_line(label, value).await;
            }
            Command::StartSequence { sequence_id } => {
                sequencer::start(sequence_id).await;
            }
            Command::StopSequence => {
                sequencer::stop().await;
            }
        }
    }
}
