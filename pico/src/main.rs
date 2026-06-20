#![no_std]
#![no_main]

use core::str::FromStr;
use defmt::*;
use embassy_executor::{Executor, Spawner};
use embassy_net::{IpAddress, IpEndpoint, Ipv4Address, Ipv4Cidr, StaticConfigV4};
use embassy_rp::adc::Channel;
use embassy_rp::gpio::{Input, Level, Output, Pull};
use embassy_rp::multicore::{Stack, spawn_core1};
use embassy_rp::peripherals::{DMA_CH0, DMA_CH1};
use embassy_rp::pwm::Pwm;
use embassy_rp::spi::{Config, Spi};
use embassy_rp::{
    Peri,
    peripherals::{ADC, USB, WATCHDOG},
};
use embassy_rp::{bind_interrupts, dma};
use embassy_time::Duration;
use heapless::Vec;
use messages::{PICO_ADDRESS_BUILD_TIME, PICO_ADDRESS_DEFAULT, Pins};
use static_cell::StaticCell;
use {defmt_rtt as _, panic_probe as _};

mod analog_in;
mod analog_out;
mod bang_bang;
mod digital_in;
mod digital_out;
mod display;
mod inbound;
mod measurements;
mod network;
mod outbound;
mod timer;
mod usb;
mod watchdog;

mod applications;

cfg_if::cfg_if! {
    if #[cfg(feature = "water_tank")] {
    use applications::water_tank as application;
    } else {
    use applications::none as application;
    }
}

bind_interrupts!(struct Irqs {
    DMA_IRQ_0 => dma::InterruptHandler<DMA_CH0>, dma::InterruptHandler<DMA_CH1>;
});

#[cortex_m_rt::entry]
fn main() -> ! {
    let p = embassy_rp::init(Default::default());

    let pins_di = Pins::from_array([
        (18, Input::new(p.PIN_18, Pull::None)),
        (19, Input::new(p.PIN_19, Pull::None)),
        (20, Input::new(p.PIN_20, Pull::None)),
        (21, Input::new(p.PIN_21, Pull::None)),
    ]);

    let pins_do = Pins::from_array([
        (10, Output::new(p.PIN_10, Level::Low)),
        (11, Output::new(p.PIN_11, Level::Low)),
        (12, Output::new(p.PIN_12, Level::Low)),
        (13, Output::new(p.PIN_13, Level::Low)),
        (25, Output::new(p.PIN_25, Level::Low)),
    ]);

    let pins_ai = Pins::from_array([
        (26, Channel::new_pin(p.PIN_26, Pull::None)),
        (27, Channel::new_pin(p.PIN_27, Pull::None)),
        (28, Channel::new_pin(p.PIN_28, Pull::None)),
    ]);

    let pwm = analog_out::configuation(100);
    let pins_ao = Pins::from_array([
        (6, Pwm::new_output_a(p.PWM_SLICE3, p.PIN_6, pwm.clone())),
        (8, Pwm::new_output_a(p.PWM_SLICE4, p.PIN_8, pwm.clone())),
    ]);

    let spi0 = Spi::new_txonly(p.SPI0, p.PIN_2, p.PIN_3, p.DMA_CH0, Irqs, Config::default());
    let display = display::Config {
        spi0: spi0,
        reset: Output::new(p.PIN_0, Level::High),
        d_c: Output::new(p.PIN_1, Level::Low),
        cs: Output::new(p.PIN_5, Level::Low),
    };

    static CORE1_STACK: StaticCell<Stack<8192>> = StaticCell::new();
    let mut core1_stack = CORE1_STACK.init(Stack::new());

    static EXECUTOR0: StaticCell<Executor> = StaticCell::new();
    static EXECUTOR1: StaticCell<Executor> = StaticCell::new();

    spawn_core1(
        p.CORE1,
        unsafe { &mut *core::ptr::addr_of_mut!(core1_stack) },
        || {
            EXECUTOR1.init(Executor::new()).run(|spawner| {
                core1_task(spawner, p.ADC, pins_di, pins_do, pins_ai, pins_ao, display);
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

    let (network, network_stack) = network::new_network(network_card, network_settings.config);
    let dhcp_server = network_stack.add_dhcp_server();

    spawner.spawn(unwrap!(usb::usb_task(usb_device)));
    spawner.spawn(unwrap!(usb::ethernet_task(ethernet)));
    spawner.spawn(unwrap!(usb::logging_task(logging)));

    spawner.spawn(unwrap!(network::dhcp_task(dhcp_server, network_stack)));
    spawner.spawn(unwrap!(network::network_task(network)));
    spawner.spawn(unwrap!(network::notify_when_available(network_stack)));

    spawner.spawn(unwrap!(watchdog::task(watchdog, Duration::from_secs(3))));
    spawner.spawn(unwrap!(inbound::task(network_stack, network_settings.port)));

    spawner.spawn(unwrap!(outbound::task(network_stack)));
}

fn core1_task(
    spawner: Spawner,
    adc: Peri<'static, ADC>,
    pins_di: Pins<Input<'static>>,
    pins_do: Pins<Output<'static>>,
    pins_ai: Pins<Channel<'static>>,
    pins_ao: Pins<Pwm<'static>>,
    display: display::Config,
) {
    spawner.spawn(unwrap!(digital_in::task(
        Duration::from_millis(1000),
        pins_di,
    )));
    spawner.spawn(unwrap!(digital_out::task(
        Duration::from_millis(1000),
        pins_do,
    )));
    spawner.spawn(unwrap!(analog_in::task(
        Duration::from_millis(1000),
        adc,
        pins_ai,
    )));
    spawner.spawn(unwrap!(analog_out::task(
        Duration::from_millis(1000),
        pins_ao,
    )));
    spawner.spawn(unwrap!(measurements::task()));
    spawner.spawn(unwrap!(display::task(display)));
    spawner.spawn(unwrap!(bang_bang::task(Duration::from_millis(1000))));
    spawner.spawn(unwrap!(application::task()));
}

struct NetworkSettings {
    config: StaticConfigV4,
    port: u16,
}

impl NetworkSettings {
    fn new() -> Self {
        let address = match PICO_ADDRESS_BUILD_TIME {
            Some(value) => value,
            None => PICO_ADDRESS_DEFAULT,
        };

        let ([a, b, c, d], port) = Self::parse_address(address);
        let config = embassy_net::StaticConfigV4 {
            address: Ipv4Cidr::new(Ipv4Address::new(a, b, c, d), 24),
            dns_servers: Vec::new(),
            gateway: Some(Ipv4Address::new(a, b, c, d + 1)),
        };
        NetworkSettings { config, port }
    }

    fn parse_address(address: &str) -> ([u8; 4], u16) {
        let Ok(endpoint) = IpEndpoint::from_str(address) else {
            return Default::default();
        };

        let IpAddress::Ipv4(address) = endpoint.addr else {
            return Default::default();
        };

        (address.octets(), endpoint.port)
    }
}
