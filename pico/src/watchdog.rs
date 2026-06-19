use embassy_futures::select::{Either, select};
use embassy_rp::Peri;
use embassy_rp::peripherals::WATCHDOG;
use embassy_rp::watchdog::Watchdog;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embassy_sync::signal::Signal;
use embassy_time::Duration;
use {defmt_rtt as _, panic_probe as _};

use crate::network;

enum Message {
    Restart,
}

static NOTIFICATION: Signal<CriticalSectionRawMutex, ()> = Signal::new();
static INBOX: Channel<CriticalSectionRawMutex, Message, 16> = Channel::new();

pub fn notify() {
    NOTIFICATION.signal(());
}

pub async fn restart() {
    INBOX.send(Message::Restart).await;
}

#[embassy_executor::task]
pub async fn task(watchdog: Peri<'static, WATCHDOG>, timeout: Duration) {
    network::wait_for_network().await;

    let mut watchdog = Watchdog::new(watchdog);
    watchdog.start(timeout);

    loop {
        match select(NOTIFICATION.wait(), INBOX.receive()).await {
            Either::First(()) => watchdog.feed(timeout),
            Either::Second(Message::Restart) => watchdog.trigger_reset(),
        };
    }
}
