use embassy_futures::select::{Either, select};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use messages::{BangBang, Content, Diagnostics};
use {defmt_rtt as _, panic_probe as _};

use crate::measurements::Measurements;
use crate::network;
use crate::outbound;

mod execution;
mod types;

use execution::Execution;
pub use types::{Guard, Sequence, Step};

enum Message {
    Start { sequence_id: u8 },
    Stop,

    Measurements { measurements: Measurements },
}

static INBOX: Channel<CriticalSectionRawMutex, Message, 16> = Channel::new();

pub async fn notify(measurements: &Measurements) {
    let measurements = *measurements;
    INBOX.send(Message::Measurements { measurements }).await;
}

pub async fn start(sequence_id: u8) {
    INBOX.send(Message::Start { sequence_id }).await;
}

pub async fn stop() {
    INBOX.send(Message::Stop).await;
}

#[embassy_executor::task]
pub async fn task(sequences: &'static [Sequence<'static>]) {
    network::wait_for_network().await;

    let mut measurements = Measurements::default();

    let execution = Execution::new();
    loop {
        match select(execution.execute_step(&measurements), INBOX.receive()).await {
            Either::First(_) => {
                // Timer::after(Duration::from_secs(1)).await;
            }
            Either::Second(Message::Start { sequence_id }) => {
                if let Some(sequence) = sequences.iter().find(|sequence| sequence.id == sequence_id)
                {
                    execution.start(sequence).await;
                } else {
                    log::info!("sequencer: unknown sequence {}", sequence_id);
                };
            }
            Either::Second(Message::Stop) => {
                execution.stop().await;
            }
            Either::Second(Message::Measurements {
                measurements: new_values,
            }) => {
                measurements = new_values;
            }
        };
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
