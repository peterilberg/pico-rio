use embassy_rp::Peri;
use embassy_rp::peripherals::WATCHDOG;
use embassy_rp::watchdog::Watchdog;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::signal;
use embassy_time::Duration;

use crate::network;

use {defmt_rtt as _, panic_probe as _};

static ALIVE: signal::Signal<CriticalSectionRawMutex, ()> = signal::Signal::new();

#[embassy_executor::task]
pub async fn watchdog_task(watchdog: Peri<'static, WATCHDOG>) {
    network::wait_for_network().await;

    let mut watchdog = Watchdog::new(watchdog);
    watchdog.start(Duration::from_secs(3));

    loop {
        ALIVE.wait().await;
        log::info!("still alive");
        watchdog.feed(Duration::from_secs(3));
    }
}

pub fn feed() {
    ALIVE.signal(());
}
