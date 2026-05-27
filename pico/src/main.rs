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
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;

use heapless::Vec;
use messages::{Diagnostics, Notification};
use static_cell::StaticCell;
use {defmt_rtt as _, panic_probe as _};

use embassy_sync::channel::Channel;

mod command;
mod network;
mod notification;
mod usb;
mod watchdog;

mod digital_out;

type Packet = (IpEndpoint, Notification, Diagnostics);

type Mailbox<T> = Channel<CriticalSectionRawMutex, T, 16>;

#[cortex_m_rt::entry]
fn main() -> ! {
    let p = embassy_rp::init(Default::default());

    let _null_endpoint = IpEndpoint::new(
        embassy_net::IpAddress::Ipv4(Ipv4Address::new(0, 0, 0, 0)),
        0,
    );

    static CORE1_STACK: StaticCell<Stack<8192>> = StaticCell::new();
    let mut core1_stack = CORE1_STACK.init(Stack::new());

    static EXECUTOR0: StaticCell<Executor> = StaticCell::new();
    static EXECUTOR1: StaticCell<Executor> = StaticCell::new();

    static DIGITAL_OUT_MAILBOX: Mailbox<digital_out::Message> =
        Mailbox::<digital_out::Message>::new();
    let do_receiver = DIGITAL_OUT_MAILBOX.receiver();
    let do_sender = DIGITAL_OUT_MAILBOX.sender();

    static NETWORK_MAILBOX: Mailbox<(IpEndpoint, messages::Notification, messages::Diagnostics)> =
        Mailbox::<(IpEndpoint, messages::Notification, messages::Diagnostics)>::new();
    let net_receiver = NETWORK_MAILBOX.receiver();
    let net_sender = NETWORK_MAILBOX.sender();

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
                    net_sender,
                )));
            });
        },
    );

    let executor0 = EXECUTOR0.init(Executor::new());
    executor0.run(|spawner| {
        let mut usb = usb::get_device(p.USB);
        let logging = usb.add_logging();
        let (ethernet, network_card) = usb.add_ethernet();

        let config = embassy_net::StaticConfigV4 {
            address: Ipv4Cidr::new(Ipv4Address::new(192, 168, 7, 1), 24),
            dns_servers: Vec::new(),
            gateway: Some(Ipv4Address::new(192, 168, 7, 2)),
        };
        let (stack, runner) = network::new_network_stack(network_card, config);
        let dhcp_server = stack.add_dhcp_server();

        spawner.spawn(unwrap!(usb::usb_task(usb)));
        spawner.spawn(unwrap!(usb::ethernet_task(ethernet)));
        spawner.spawn(unwrap!(usb::logging_task(logging)));

        spawner.spawn(unwrap!(network::dhcp_task(dhcp_server)));
        spawner.spawn(unwrap!(network::net_task(runner)));
        spawner.spawn(unwrap!(network::notify_when_network_is_available(
            network::NStack(stack.0)
        )));

        spawner.spawn(unwrap!(watchdog::watchdog_task(p.WATCHDOG,)));
        spawner.spawn(unwrap!(command::udp_input_task(
            network::NStack(stack.0),
            do_sender
        )));
        spawner.spawn(unwrap!(notification::udp_output_task(stack, net_receiver)));
    });
}
