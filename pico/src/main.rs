#![no_std]
#![no_main]

use defmt::*;
use embassy_executor::{Executor, Spawner};
use embassy_net::{IpEndpoint, Ipv4Address, Ipv4Cidr};
use embassy_rp::gpio::{Input, Level, Output, Pull};
use embassy_rp::multicore::{Stack, spawn_core1};
use embassy_rp::{
    Peri,
    peripherals::{USB, WATCHDOG},
};
use embassy_time::Duration;
use heapless::Vec;
use messages::{NUM_PINS_DI, NUM_PINS_DO};
use static_cell::StaticCell;
use {defmt_rtt as _, panic_probe as _};

use crate::mailbox::Mailbox;

mod digital_in;
mod digital_out;
mod inbound;
mod mailbox;
mod network;
mod outbound;
mod timer;
mod usb;
mod watchdog;

static OUTBOUND: Mailbox<outbound::Message> = Mailbox::<outbound::Message>::new();
static DIGITAL_OUT: Mailbox<digital_out::Message> = Mailbox::<digital_out::Message>::new();

#[cortex_m_rt::entry]
fn main() -> ! {
    let p = embassy_rp::init(Default::default());

    let pins_di = [
        (18, Input::new(p.PIN_18, Pull::None)),
        (19, Input::new(p.PIN_19, Pull::None)),
        (20, Input::new(p.PIN_20, Pull::None)),
        (21, Input::new(p.PIN_21, Pull::None)),
    ];

    let pins_do = [
        (10, Output::new(p.PIN_10, Level::Low)),
        (11, Output::new(p.PIN_11, Level::Low)),
        (12, Output::new(p.PIN_12, Level::Low)),
        (13, Output::new(p.PIN_13, Level::Low)),
        (25, Output::new(p.PIN_25, Level::Low)),
    ];

    static CORE1_STACK: StaticCell<Stack<8192>> = StaticCell::new();
    let mut core1_stack = CORE1_STACK.init(Stack::new());

    static EXECUTOR0: StaticCell<Executor> = StaticCell::new();
    static EXECUTOR1: StaticCell<Executor> = StaticCell::new();

    spawn_core1(
        p.CORE1,
        unsafe { &mut *core::ptr::addr_of_mut!(core1_stack) },
        || {
            EXECUTOR1.init(Executor::new()).run(|spawner| {
                core1_task(spawner, pins_di, pins_do);
            });
        },
    );

    EXECUTOR0.init(Executor::new()).run(|spawner| {
        core0_task(spawner, p.USB, p.WATCHDOG, NetworkSettings::new());
    });
}

fn core0_task(
    spawner: Spawner,
    usb: Peri<'static, USB>,
    watchdog: Peri<'static, WATCHDOG>,
    network_settings: NetworkSettings,
) {
    let mut usb_device = usb::get_device(usb);
    let logging = usb_device.add_logging();
    let (ethernet, network_card) = usb_device.add_ethernet();

    let [a, b, c, d] = network_settings.address.octets();
    let configuration = embassy_net::StaticConfigV4 {
        address: Ipv4Cidr::new(network_settings.address, 24),
        dns_servers: Vec::new(),
        gateway: Some(Ipv4Address::new(a, b, c, d + 1)),
    };
    let (network, network_stack) = network::new_network(network_card, configuration);
    let dhcp_server = network_stack.add_dhcp_server();

    spawner.spawn(unwrap!(usb::usb_task(usb_device)));
    spawner.spawn(unwrap!(usb::ethernet_task(ethernet)));
    spawner.spawn(unwrap!(usb::logging_task(logging)));

    spawner.spawn(unwrap!(network::dhcp_task(dhcp_server, network_stack)));
    spawner.spawn(unwrap!(network::network_task(network)));
    spawner.spawn(unwrap!(network::notify_when_available(network_stack)));

    spawner.spawn(unwrap!(watchdog::task(watchdog, Duration::from_secs(3))));
    spawner.spawn(unwrap!(inbound::task(
        network_stack,
        network_settings.port,
        DIGITAL_OUT.sender()
    )));

    spawner.spawn(unwrap!(outbound::task(
        network_stack,
        network_settings.destination,
        OUTBOUND.receiver()
    )));
}

fn core1_task(
    spawner: Spawner,
    pins_di: [(u8, Input<'static>); NUM_PINS_DI],
    pins_do: [(u8, Output<'static>); NUM_PINS_DO],
) {
    spawner.spawn(unwrap!(digital_in::task(
        Duration::from_millis(1000),
        pins_di,
        OUTBOUND.sender(),
    )));
    spawner.spawn(unwrap!(digital_out::task(
        Duration::from_millis(1000),
        pins_do,
        DIGITAL_OUT.receiver(),
        OUTBOUND.sender(),
    )));
}

struct NetworkSettings {
    address: Ipv4Address,
    port: u16,
    destination: IpEndpoint,
}

impl NetworkSettings {
    fn new() -> Self {
        let address = Ipv4Address::new(192, 168, 7, 1);
        let port = 1234;

        let destination_address = Ipv4Address::new(192, 168, 64, 47);
        let destination_port = 12345;

        NetworkSettings {
            address,
            port,
            destination: IpEndpoint::new(
                embassy_net::IpAddress::Ipv4(destination_address),
                destination_port,
            ),
        }
    }
}
