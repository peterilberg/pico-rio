use embassy_futures::select::{Either, select};
use embassy_rp::gpio::{Level, Output};
use embassy_time::{Duration, Instant, Ticker};

use heapless::{LinearMap, Vec};
use messages::{Content, Diagnostics};
use serde::{Deserialize, Serialize};

use {defmt_rtt as _, panic_probe as _};

use crate::mailbox::{Inbox, Outbox};
use crate::network;
use crate::outbound;
use crate::watchdog;

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
pub enum Message {
    Set { pin: u8, value: bool },
}

#[embassy_executor::task]
pub async fn task(
    interval_ms: u32,
    pins2: [(u8, Output<'static>); 1],
    inbox: Inbox<Message>,
    outbound: Outbox<outbound::Message>,
) {
    network::wait_for_network().await;

    let mut pins: LinearMap<_, _, 1> = LinearMap::new();
    for (k, v) in pins2 {
        pins.insert(k, v).unwrap();
    }

    let mut foo = Foo::new(Duration::from_millis(interval_ms.into()), inbox);

    loop {
        log::info!("digital out 0");
        match loop_check(&mut foo).await {
            None => {
                log::info!(
                    "do times: exec {} jitter {}",
                    foo.diag.execution_us,
                    foo.diag.jitter_in_us,
                );
                let pins: Vec<_, 1> = pins
                    .iter()
                    .map(|(n, pin)| {
                        (
                            *n,
                            match pin.get_output_level() {
                                Level::Low => false,
                                Level::High => true,
                            },
                        )
                    })
                    .collect();
                let pins = *pins.as_array().unwrap();
                let info = outbound::Message {
                    content: Content::DO { pins },
                    diagnostics: Diagnostics { ..foo.diag },
                };

                log::info!("digital out 5");
                outbound.send(info).await;
                log::info!("digital out 6");
            }
            Some(Message::Set { pin, value }) => {
                log::info!("digital out 1");
                match pins.get_mut(&pin) {
                    None => log::info!("unknown pin {}", 25),
                    Some(pin) => {
                        pin.set_level(match value {
                            false => Level::Low,
                            true => Level::High,
                        });
                    }
                }
                log::info!("digital out 2");
            }
        }

        watchdog::notify();
    }
}

async fn loop_check<T: 'static>(foo: &mut Foo<T>) -> Option<T> {
    foo.update();

    let msg = match select(foo.seconds.next(), foo.receiver.receive()).await {
        Either::First(()) => None,
        Either::Second(msg) => Some(msg),
    };

    foo.update2();
    msg
}

struct Foo<T: 'static> {
    seconds: Ticker,
    receiver: Inbox<T>,
    start_time: Instant,
    diag: Diagnostics,
}

impl<T> Foo<T> {
    fn new(duration: Duration, receiver: Inbox<T>) -> Self {
        let seconds = Ticker::every(Duration::from_secs(1));
        Foo {
            seconds,
            receiver,
            start_time: Instant::now(),
            diag: Diagnostics {
                execution_us: 0,
                jitter_in_us: 0,
                period_in_us: duration.as_micros(),
                timestamp_us: 0,
            },
        }
    }

    fn update(&mut self) {
        let end_time = Instant::now();
        self.diag.execution_us = end_time
            .checked_duration_since(self.start_time)
            .unwrap_or(Duration::from_secs(0))
            .as_micros();
    }

    fn update2(&mut self) {
        let jitter = match self.start_time.checked_add(Duration::from_secs(1)) {
            None => Duration::from_secs(0),
            Some(expected) => Instant::now()
                .checked_duration_since(expected)
                .unwrap_or(Duration::from_secs(0)),
        };
        self.diag.jitter_in_us = jitter.as_micros();
        // TODO period
        self.start_time = Instant::now();
        self.diag.timestamp_us = self.start_time.as_micros();
    }
}

// TODO must be able to support multiple digital output tasks differing in pins
// static digital output tasks, static cells with init setting pins
// service hides barrier watch be start_services function.
// Service trait, task function from service trait to task?
// -> does not work because task cannot be generic, must be concrete.
// -> do we really need a generic task function? or specific ones for each task?
// task, send_to_? well we need to be able to identify service
// export service struct with task and send to.
