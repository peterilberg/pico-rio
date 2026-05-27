use embassy_rp::Peri;
use embassy_rp::bind_interrupts;
use embassy_rp::peripherals::USB;
use embassy_rp::usb::{Driver, InterruptHandler};
use embassy_usb::class::cdc_acm::{CdcAcmClass, State as AcmState};
use embassy_usb::class::cdc_ncm::embassy_net::{Device, Runner, State};
use embassy_usb::class::cdc_ncm::{CdcNcmClass, State as NcmState};
use embassy_usb::{Builder, Config};

use static_cell::StaticCell;
use {defmt_rtt as _, panic_probe as _};

bind_interrupts!(struct Irqs {
    USBCTRL_IRQ => InterruptHandler<USB>;
});

const MTU: usize = 1514;

pub struct Usb(Builder<'static, Driver<'static, USB>>);
pub struct Logging(CdcAcmClass<'static, Driver<'static, USB>>);
pub struct Ethernet(Runner<'static, Driver<'static, USB>, MTU>);

pub type NetworkCard = Device<'static, MTU>;

pub fn get_device(usb: Peri<'static, USB>) -> Usb {
    let driver = Driver::new(usb, Irqs);

    // Create embassy-usb Config
    let mut config = Config::new(0xc0de, 0xcafe);
    config.manufacturer = Some("Embassy");
    config.product = Some("USB-Ethernet example");
    config.serial_number = Some("12345678");
    config.max_power = 100;
    config.max_packet_size_0 = 64;

    // Create embassy-usb DeviceBuilder using the driver and config.
    static CONFIG_DESC: StaticCell<[u8; 256]> = StaticCell::new();
    static BOS_DESC: StaticCell<[u8; 256]> = StaticCell::new();
    static CONTROL_BUF: StaticCell<[u8; 128]> = StaticCell::new();
    let builder = Builder::new(
        driver,
        config,
        &mut CONFIG_DESC.init([0; 256])[..],
        &mut BOS_DESC.init([0; 256])[..],
        &mut [], // no msos descriptors
        &mut CONTROL_BUF.init([0; 128])[..],
    );
    Usb(builder)
}

impl Usb {
    pub fn add_logging(&mut self) -> Logging {
        // Create a class for the logger
        static LOG_STATE: StaticCell<AcmState> = StaticCell::new();
        let logging = CdcAcmClass::new(&mut self.0, LOG_STATE.init(AcmState::new()), 64);
        Logging(logging)
    }

    pub fn add_ethernet(&mut self) -> (Ethernet, NetworkCard) {
        // Host's MAC addr. This is the MAC the host "thinks" its USB-to-ethernet adapter has.
        let host_mac_addr = [0x88, 0x88, 0x88, 0x88, 0x88, 0x88];

        // Our MAC addr.
        let our_mac_addr = [0xCC, 0xCC, 0xCC, 0xCC, 0xCC, 0xCC];

        // Create classes on the builder.
        static STATE: StaticCell<NcmState> = StaticCell::new();
        let class = CdcNcmClass::new(&mut self.0, STATE.init(NcmState::new()), host_mac_addr, 64);

        static NET_STATE: StaticCell<State<MTU, 4, 4>> = StaticCell::new();
        let (usbrunner, device) =
            class.into_embassy_net_device::<MTU, 4, 4>(NET_STATE.init(State::new()), our_mac_addr);
        (Ethernet(usbrunner), device)
    }

    pub async fn run(self) -> ! {
        self.0.build().run().await
    }
}

impl Logging {
    pub async fn run(self) -> ! {
        let log = self.0;
        let log_fut = embassy_usb_logger::with_class!(1024, log::LevelFilter::Info, log);
        log_fut.await
    }
}

impl Ethernet {
    pub async fn run(self) -> ! {
        self.0.run().await
    }
}

#[embassy_executor::task]
pub async fn usb_task(task: Usb) -> ! {
    task.run().await
}

#[embassy_executor::task]
pub async fn ethernet_task(task: Ethernet) -> ! {
    task.run().await
}

#[embassy_executor::task]
pub async fn logging_task(task: Logging) -> ! {
    task.run().await
}
