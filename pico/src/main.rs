//! https://github.com/embassy-rs/embassy/blob/main/examples/rp/src/bin/usb_ethernet.rs
//!
//! This example shows how to use USB (Universal Serial Bus) in the RP2040 chip.
//!
//! This is a CDC-NCM class implementation, aka Ethernet over USB.

#![no_std]
#![no_main]

use defmt::*;
use embassy_executor::Executor;
use embassy_net::{IpEndpoint, Ipv4Address, Ipv4Cidr};
use embassy_rp::gpio::{Level, Output};
use embassy_rp::multicore::{Stack, spawn_core1};

use embassy_time::Duration;
use heapless::Vec;
use static_cell::StaticCell;
use {defmt_rtt as _, panic_probe as _};

use crate::mailbox::Mailbox;

mod digital_out;
mod inbound;
mod mailbox;
mod network;
mod outbound;
mod usb;
mod watchdog;

#[cortex_m_rt::entry]
fn main() -> ! {
    let p = embassy_rp::init(Default::default());

    static CORE1_STACK: StaticCell<Stack<8192>> = StaticCell::new();
    let mut core1_stack = CORE1_STACK.init(Stack::new());

    static EXECUTOR0: StaticCell<Executor> = StaticCell::new();
    static EXECUTOR1: StaticCell<Executor> = StaticCell::new();

    static DIGITAL_OUT_MAILBOX: Mailbox<digital_out::Message> =
        Mailbox::<digital_out::Message>::new();
    let do_receiver = DIGITAL_OUT_MAILBOX.receiver();
    let do_sender = DIGITAL_OUT_MAILBOX.sender();

    static NETWORK_MAILBOX: Mailbox<outbound::Message> = Mailbox::<outbound::Message>::new();
    let inbound = NETWORK_MAILBOX.receiver();
    let outbound = NETWORK_MAILBOX.sender();

    spawn_core1(
        p.CORE1,
        unsafe { &mut *core::ptr::addr_of_mut!(core1_stack) },
        move || {
            let executor1 = EXECUTOR1.init(Executor::new());
            executor1.run(|spawner| {
                spawner.spawn(unwrap!(digital_out::task(
                    100,
                    [(25, Output::new(p.PIN_25, Level::Low))],
                    do_receiver,
                    outbound,
                )));
            });
        },
    );

    let executor0 = EXECUTOR0.init(Executor::new());
    executor0.run(|spawner| {
        let mut usb_device = usb::get_device(p.USB);
        let logging = usb_device.add_logging();
        let (ethernet, network_card) = usb_device.add_ethernet();

        let config = embassy_net::StaticConfigV4 {
            address: Ipv4Cidr::new(Ipv4Address::new(192, 168, 7, 1), 24),
            dns_servers: Vec::new(),
            gateway: Some(Ipv4Address::new(192, 168, 7, 2)),
        };
        let (network, network_stack) = network::new_network(network_card, config);
        let dhcp_server = network_stack.add_dhcp_server();

        let default_endpoint = IpEndpoint::new(
            embassy_net::IpAddress::Ipv4(Ipv4Address::new(192, 168, 64, 47)),
            12345,
        );

        spawner.spawn(unwrap!(usb::usb_task(usb_device)));
        spawner.spawn(unwrap!(usb::ethernet_task(ethernet)));
        spawner.spawn(unwrap!(usb::logging_task(logging)));

        spawner.spawn(unwrap!(network::dhcp_task(dhcp_server, network_stack)));
        spawner.spawn(unwrap!(network::network_task(network)));
        spawner.spawn(unwrap!(network::notify_when_available(network_stack)));

        spawner.spawn(unwrap!(watchdog::task(p.WATCHDOG, Duration::from_secs(3))));
        spawner.spawn(unwrap!(inbound::task(network_stack, 1234, do_sender)));

        spawner.spawn(unwrap!(outbound::task(
            network_stack,
            default_endpoint,
            inbound
        )));
    });
}
