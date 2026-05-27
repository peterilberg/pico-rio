use embassy_rp::usb::{Driver, InterruptHandler};
use embassy_rp::{Peri, bind_interrupts, peripherals::USB};
use embassy_usb::class::cdc_acm::{CdcAcmClass, State as LoggingState};
use embassy_usb::class::cdc_ncm::embassy_net::{
    Device as NetworkDevice, Runner as NetworkDriver, State as NetworkState,
};
use embassy_usb::class::cdc_ncm::{CdcNcmClass, State as EthernetState};
use embassy_usb::{Builder, Config};
use static_cell::StaticCell;
use {defmt_rtt as _, panic_probe as _};

bind_interrupts!(struct Irqs {
    USBCTRL_IRQ => InterruptHandler<USB>;
});

const MTU: usize = 1514;

pub struct UsbDevice(Builder<'static, Driver<'static, USB>>);

pub type Logging = CdcAcmClass<'static, Driver<'static, USB>>;
pub type Ethernet = NetworkDriver<'static, Driver<'static, USB>, MTU>;
pub type NetworkCard = NetworkDevice<'static, MTU>;

pub fn get_device(usb: Peri<'static, USB>) -> UsbDevice {
    let driver = Driver::new(usb, Irqs);

    let mut config = Config::new(0xc0de, 0xcafe);
    config.manufacturer = Some("picoRIO");
    config.product = Some("picoRIO");
    config.serial_number = Some("12345678");
    config.max_power = 100;
    config.max_packet_size_0 = 64;

    static CONFIG_DESC: StaticCell<[u8; 256]> = StaticCell::new();
    static BOS_DESC: StaticCell<[u8; 256]> = StaticCell::new();
    static CONTROL_BUF: StaticCell<[u8; 128]> = StaticCell::new();

    UsbDevice(Builder::new(
        driver,
        config,
        &mut CONFIG_DESC.init([0; 256])[..],
        &mut BOS_DESC.init([0; 256])[..],
        &mut [], // no msos descriptors
        &mut CONTROL_BUF.init([0; 128])[..],
    ))
}

impl UsbDevice {
    pub fn add_logging(&mut self) -> Logging {
        let UsbDevice(usb) = self;

        static STATE: StaticCell<LoggingState> = StaticCell::new();
        CdcAcmClass::new(usb, STATE.init(LoggingState::new()), 64)
    }

    pub fn add_ethernet(&mut self) -> (Ethernet, NetworkCard) {
        let UsbDevice(usb) = self;

        let mac_address = [0xCC, 0xCC, 0xCC, 0xCC, 0xCC, 0xCC];
        let host_mac_address = [0x88, 0x88, 0x88, 0x88, 0x88, 0x88];

        static STATE: StaticCell<EthernetState> = StaticCell::new();
        let ethernet =
            CdcNcmClass::new(usb, STATE.init(EthernetState::new()), host_mac_address, 64);

        static NETWORK: StaticCell<NetworkState<MTU, 4, 4>> = StaticCell::new();
        ethernet
            .into_embassy_net_device::<MTU, 4, 4>(NETWORK.init(NetworkState::new()), mac_address)
    }
}

#[embassy_executor::task]
pub async fn usb_task(usb: UsbDevice) -> ! {
    let UsbDevice(builder) = usb;
    builder.build().run().await;
}

#[embassy_executor::task]
pub async fn ethernet_task(ethernet: Ethernet) -> ! {
    ethernet.run().await;
}

#[embassy_executor::task]
pub async fn logging_task(logging: Logging) -> ! {
    embassy_usb_logger::with_class!(1024, log::LevelFilter::Info, logging).await;
}
