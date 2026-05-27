use embassy_rp::Peri;
use embassy_rp::peripherals::WATCHDOG;
use embassy_rp::watchdog::Watchdog;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::signal::Signal;
use embassy_time::Duration;
use {defmt_rtt as _, panic_probe as _};

use crate::network;

static NOTIFICATION: Signal<CriticalSectionRawMutex, ()> = Signal::new();

#[embassy_executor::task]
pub async fn task(watchdog: Peri<'static, WATCHDOG>, timeout: Duration) {
    network::wait_for_network().await;

    let mut watchdog = Watchdog::new(watchdog);
    watchdog.start(timeout);

    loop {
        NOTIFICATION.wait().await;
        watchdog.feed(timeout);
    }
}

pub fn notify() {
    NOTIFICATION.signal(());
}
