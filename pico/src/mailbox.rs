use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::{Channel, Receiver, Sender};
use {defmt_rtt as _, panic_probe as _};

pub type Mailbox<T> = Channel<CriticalSectionRawMutex, T, 16>;
pub type Inbox<T> = Receiver<'static, CriticalSectionRawMutex, T, 16>;
pub type Outbox<T> = Sender<'static, CriticalSectionRawMutex, T, 16>;
