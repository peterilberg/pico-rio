use embassy_futures::select::{Either, select};
use embassy_net::{IpEndpoint, Ipv4Address};
use embassy_rp::gpio::{Level, Output};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_time::Duration;
use embassy_time::Instant;
use embassy_time::Ticker;

use crate::network;
use crate::watchdog;
use heapless::{LinearMap, Vec};
use messages::{Diagnostics, Notification};
use serde::{Deserialize, Serialize};

use {defmt_rtt as _, panic_probe as _};

use embassy_sync::channel::{Receiver, Sender};

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
pub enum Message {
    Set { pin: u8, value: bool },
}

#[embassy_executor::task]
pub async fn task(
    interval_ms: u32,
    pins2: [(u8, Output<'static>); 1],
    receiver: Receiver<'static, CriticalSectionRawMutex, Message, 16>,
    net_out: Sender<
        'static,
        CriticalSectionRawMutex,
        (IpEndpoint, messages::Notification, messages::Diagnostics),
        16,
    >,
) {
    network::wait_for_network().await;

    let remote_endpoint = IpEndpoint::new(
        embassy_net::IpAddress::Ipv4(Ipv4Address::new(192, 168, 64, 47)),
        12345,
    );

    let mut pins: LinearMap<_, _, 1> = LinearMap::new();
    for (k, v) in pins2 {
        pins.insert(k, v).unwrap();
    }

    let mut foo = Foo::new(Duration::from_millis(interval_ms.into()), receiver);

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
                let header = Notification::DO { pins };
                let diagnostics = Diagnostics { ..foo.diag };

                log::info!("digital out 5");
                net_out.send((remote_endpoint, header, diagnostics)).await;
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

        watchdog::feed();
    }
}

async fn loop_check<'t, T>(foo: &mut Foo<'t, T>) -> Option<T> {
    foo.update();

    let msg = match select(foo.seconds.next(), foo.receiver.receive()).await {
        Either::First(()) => None,
        Either::Second(msg) => Some(msg),
    };

    foo.update2();
    msg
}

struct Foo<'t, T> {
    seconds: Ticker,
    receiver: Receiver<'t, CriticalSectionRawMutex, T, 16>,
    start_time: Instant,
    diag: Diagnostics,
}

impl<'t, T> Foo<'t, T> {
    fn new(
        duration: Duration,
        receiver: Receiver<'static, CriticalSectionRawMutex, T, 16>,
    ) -> Self {
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
