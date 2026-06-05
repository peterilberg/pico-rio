use messages::{Diagnostics, NUM_PINS_DO};
use std::net::{IpAddr, SocketAddr};
use std::time::Duration;

pub struct Logger<'task> {
    ip: IpAddr,
    task: &'task str,
}

impl<'task> Logger<'task> {
    pub fn new(address: SocketAddr, task: &'task str) -> Self {
        Logger {
            task,
            ip: address.ip(),
        }
    }

    pub fn error(address: SocketAddr) {
        Logger::new(address, "ERROR: ").prefix();
    }

    pub fn prefix(&self) {
        print!("{}: {} ", self.ip, self.task);
    }

    pub fn diagnostics(&self, diagnostics: Diagnostics) {
        self.prefix();
        println!(
            "at {:?} (+ {:?}) with period {:?} took {:?}",
            Duration::from_micros(diagnostics.timestamp_us),
            Duration::from_micros(diagnostics.jitter_in_us),
            Duration::from_micros(diagnostics.period_in_us),
            Duration::from_micros(diagnostics.execution_us),
        );
    }

    pub fn digital_out(&self, pins: [(u8, bool); NUM_PINS_DO], diagnostics: Diagnostics) {
        self.diagnostics(diagnostics);
        for (pin, level) in pins {
            self.prefix();
            println!(
                "pin {}: {}",
                pin,
                match level {
                    false => "off",
                    true => "on",
                }
            );
        }
    }
}
