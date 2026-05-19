//! https://github.com/embassy-rs/embassy/blob/main/examples/rp/src/bin/usb_ethernet.rs
//!
//! This example shows how to use USB (Universal Serial Bus) in the RP2040 chip.
//!
//! This is a CDC-NCM class implementation, aka Ethernet over USB.

#![no_std]
#![no_main]

use defmt::*;
use embassy_executor::Executor;
use embassy_futures::select::{Either, select};
use embassy_net::udp::{PacketMetadata, UdpSocket};
use embassy_net::{
    IpEndpoint, Ipv4Address, Ipv4Cidr, Runner as NetRunner, Stack as NetStack, StackResources,
    StaticConfigV4,
};
use embassy_rp::Peri;
use embassy_rp::clocks::RoscRng;
use embassy_rp::gpio::{Level, Output};
use embassy_rp::multicore::{Stack, spawn_core1};
use embassy_rp::peripherals::{USB, WATCHDOG};
use embassy_rp::usb::{Driver, InterruptHandler};
use embassy_rp::watchdog::Watchdog;
use embassy_rp::{bind_interrupts, peripherals};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::signal;
use embassy_sync::watch;
use embassy_time::Duration;
use embassy_time::Instant;
use embassy_time::Ticker;
use embassy_usb::class::cdc_acm::{CdcAcmClass, State as AcmState};
use embassy_usb::class::cdc_ncm::embassy_net::{Device, Runner, State as NetState};
use embassy_usb::class::cdc_ncm::{CdcNcmClass, State as NcmState};
use embassy_usb::{Builder, Config, UsbDevice};

use heapless::{LinearMap, Vec};
use leasehund::{
    DHCPServerBuffers, DHCPServerSocket, DhcpConfig as LeaseConfig, DhcpConfigBuilder, DhcpServer,
    TransactionEvent,
};
use messages::{Command, Diagnostics, Notification};
use postcard::{from_bytes, to_slice};
use serde::{Deserialize, Serialize};
use static_cell::StaticCell;
use {defmt_rtt as _, panic_probe as _};

use embassy_sync::channel::Channel;

static STATE_CHANGED: watch::Watch<CriticalSectionRawMutex, (), 32> = watch::Watch::new();
static ALIVE: signal::Signal<CriticalSectionRawMutex, ()> = signal::Signal::new();

static NETWORK_OUT: Channel<CriticalSectionRawMutex, Packet, 16> = Channel::new();

type Packet = (IpEndpoint, Notification, Diagnostics);

bind_interrupts!(struct Irqs {
    USBCTRL_IRQ => InterruptHandler<USB>;
});

type MyDriver = Driver<'static, peripherals::USB>;

const MTU: usize = 1514;

#[embassy_executor::task]
async fn usb_task(mut device: UsbDevice<'static, MyDriver>) -> ! {
    device.run().await
}

#[embassy_executor::task]
async fn usb_ncm_task(class: Runner<'static, MyDriver, MTU>) -> ! {
    class.run().await
}

#[embassy_executor::task]
async fn net_task(mut runner: embassy_net::Runner<'static, Device<'static, MTU>>) -> ! {
    runner.run().await
}

#[embassy_executor::task]
async fn log_task(logger: CdcAcmClass<'static, Driver<'static, USB>>) -> ! {
    // Creates the logger and returns the logger future
    // Note: You'll need to use log::info! afterwards instead of info! for this to work (this also applies to all the other log::* macros)
    let log_fut = embassy_usb_logger::with_class!(1024, log::LevelFilter::Info, logger);
    log_fut.await
}

#[embassy_executor::task]
async fn dhcp_task(mut server: DhcpServer<32, 4>, stack: NetStack<'static>) -> ! {
    let mut buffers = DHCPServerBuffers::new();
    let mut socket = DHCPServerSocket::new(stack, &mut buffers);
    loop {
        let Ok(event) = server.lease_one(&mut socket).await else {
            // Handle error (e.g., log it)
            log::info!("Lease failed.");
            continue;
        };

        match event {
            TransactionEvent::Leased(ip, mac) => {
                log::info!("Leased IP: {} to MAC: {:02x?}", ip, mac);
                let config = stack.config_v4().unwrap();
                stack.set_config_v4(embassy_net::ConfigV4::Static(StaticConfigV4 {
                    gateway: Some(ip),
                    ..config
                }));
                stack.wait_config_up().await;
                log::info!("Replace gateway");
            }
            TransactionEvent::Released(ip, mac) => {
                log::info!("Released IP: {} from MAC: {:02x?}", ip, mac);
            }
        }
    }
}

#[cortex_m_rt::entry]
fn main() -> ! {
    let p = embassy_rp::init(Default::default());

    let _null_endpoint = IpEndpoint::new(
        embassy_net::IpAddress::Ipv4(Ipv4Address::new(0, 0, 0, 0)),
        0,
    );

    // let meta = UdpMetadata::from(null_endpoint);
    /*
        static BUF0: StaticCell<[Packet; NUM_PACKETS]> = StaticCell::new();
        let buf0 = BUF0.init([(meta, [0; PACKET_SIZE]); NUM_PACKETS]);

        static BUF1: StaticCell<[Packet; NUM_PACKETS]> = StaticCell::new();
        let buf1 = BUF1.init([(meta, [0; PACKET_SIZE]); NUM_PACKETS]);
    */

    static CORE1_STACK: StaticCell<Stack<8192>> = StaticCell::new();
    let mut core1_stack = CORE1_STACK.init(Stack::new());

    static EXECUTOR0: StaticCell<Executor> = StaticCell::new();
    static EXECUTOR1: StaticCell<Executor> = StaticCell::new();

    spawn_core1(
        p.CORE1,
        unsafe { &mut *core::ptr::addr_of_mut!(core1_stack) },
        move || {
            let executor1 = EXECUTOR1.init(Executor::new());
            executor1.run(|spawner| {
                spawner.spawn(unwrap!(digital_output_task(
                    STATE_CHANGED.receiver().unwrap(),
                    [(25, Output::new(p.PIN_25, Level::Low))]
                )));
            });
        },
    );

    let executor0 = EXECUTOR0.init(Executor::new());
    executor0.run(|spawner| {
        let driver = get_usb_driver(p.USB);
        let mut builder = get_usb_builder(driver);
        let logger_class = get_logger(&mut builder);
        let (usbrunner, device) = get_usb_network_device(&mut builder);

        let (stack, runner) = get_network_stack(device);
        let dhcp_server = get_dhcp_server();

        spawner.spawn(unwrap!(usb_task(builder.build())));
        spawner.spawn(unwrap!(usb_ncm_task(usbrunner)));

        spawner.spawn(unwrap!(log_task(logger_class)));

        spawner.spawn(unwrap!(dhcp_task(dhcp_server, stack)));
        spawner.spawn(unwrap!(net_task(runner)));
        spawner.spawn(unwrap!(wait_for_network(stack)));

        spawner.spawn(unwrap!(watchdog_task(
            p.WATCHDOG,
            STATE_CHANGED.receiver().unwrap(),
        )));
        spawner.spawn(unwrap!(udp_input_task(
            stack,
            STATE_CHANGED.receiver().unwrap(),
        )));
        spawner.spawn(unwrap!(udp_output_task(
            stack,
            STATE_CHANGED.receiver().unwrap(),
        )));
    });
}

fn get_usb_driver(usb: Peri<'static, USB>) -> Driver<'static, USB> {
    // Create the driver, from the HAL.
    Driver::new(usb, Irqs)
}

fn get_usb_builder(driver: Driver<'static, USB>) -> Builder<'static, Driver<'static, USB>> {
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
    Builder::new(
        driver,
        config,
        &mut CONFIG_DESC.init([0; 256])[..],
        &mut BOS_DESC.init([0; 256])[..],
        &mut [], // no msos descriptors
        &mut CONTROL_BUF.init([0; 128])[..],
    )
}

fn get_logger(
    builder: &mut Builder<'static, Driver<'static, USB>>,
) -> CdcAcmClass<'static, Driver<'static, USB>> {
    // Create a class for the logger
    static LOG_STATE: StaticCell<AcmState> = StaticCell::new();
    CdcAcmClass::new(builder, LOG_STATE.init(AcmState::new()), 64)
}

fn get_usb_network_device(
    builder: &mut Builder<'static, Driver<'static, USB>>,
) -> (
    Runner<'static, Driver<'static, USB>, MTU>,
    Device<'static, MTU>,
) {
    // Host's MAC addr. This is the MAC the host "thinks" its USB-to-ethernet adapter has.
    let host_mac_addr = [0x88, 0x88, 0x88, 0x88, 0x88, 0x88];

    // Our MAC addr.
    let our_mac_addr = [0xCC, 0xCC, 0xCC, 0xCC, 0xCC, 0xCC];

    // Create classes on the builder.
    static STATE: StaticCell<NcmState> = StaticCell::new();
    let class = CdcNcmClass::new(builder, STATE.init(NcmState::new()), host_mac_addr, 64);

    static NET_STATE: StaticCell<NetState<MTU, 4, 4>> = StaticCell::new();
    class.into_embassy_net_device::<MTU, 4, 4>(NET_STATE.init(NetState::new()), our_mac_addr)
}

fn get_network_stack(
    device: Device<'static, MTU>,
) -> (NetStack<'static>, NetRunner<'static, Device<'static, MTU>>) {
    //let hostname = String::<32/*MAX_HOSTNAME_LEN*/>::try_from("picoRIO").unwrap();
    //let mut dhcp_config = DhcpConfig::default();
    //dhcp_config.hostname = Some(hostname);
    // let config = embassy_net::Config::dhcpv4(DhcpConfig::default());
    let config = embassy_net::Config::ipv4_static(embassy_net::StaticConfigV4 {
        address: Ipv4Cidr::new(Ipv4Address::new(192, 168, 7, 1), 24),
        dns_servers: Vec::new(),
        gateway: Some(Ipv4Address::new(192, 168, 7, 2)),
    });

    // Generate random seed
    let mut rng = RoscRng;
    let seed = rng.next_u64();

    // Init network stack
    static RESOURCES: StaticCell<StackResources<10>> = StaticCell::new();
    embassy_net::new(device, config, RESOURCES.init(StackResources::new()), seed)
}

fn get_dhcp_server() -> DhcpServer<32, 4> {
    let config: LeaseConfig<4> = DhcpConfigBuilder::new()
        .server_ip(Ipv4Address::new(192, 168, 7, 1))
        .subnet_mask(Ipv4Address::new(255, 255, 255, 0))
        .router(Ipv4Address::new(192, 168, 7, 2))
        .add_dns_server(Ipv4Address::new(1, 1, 1, 1)) // Cloudflare DNS
        .add_dns_server(Ipv4Address::new(1, 0, 0, 1)) // Cloudflare backup
        .add_dns_server(Ipv4Address::new(8, 8, 8, 8)) // Google DNS
        .ip_pool(
            Ipv4Address::new(192, 168, 7, 100),
            Ipv4Address::new(192, 168, 7, 200),
        )
        .lease_time(7200) // 2 hours
        .build();

    DhcpServer::with_config(config)
}

#[embassy_executor::task]
async fn wait_for_network(stack: NetStack<'static>) {
    log::info!("waiting for config up");
    stack.wait_config_up().await;

    // And now we can use it!
    STATE_CHANGED.sender().send(());
}

#[embassy_executor::task]
async fn watchdog_task(
    watchdog: Peri<'static, WATCHDOG>,
    mut watch: watch::Receiver<'static, CriticalSectionRawMutex, (), 32>,
) {
    watch.changed().await;

    let mut watchdog = Watchdog::new(watchdog);
    watchdog.start(Duration::from_secs(3));

    loop {
        ALIVE.wait().await;
        log::info!("still alive");
        watchdog.feed(Duration::from_secs(3));
    }
}

#[embassy_executor::task]
async fn udp_input_task(
    stack: NetStack<'static>,
    mut watch: watch::Receiver<'static, CriticalSectionRawMutex, (), 32>,
) {
    watch.changed().await;

    let mut rx_buffer = [0; 4096];
    let mut tx_buffer = [0; 4096];
    let mut rx_meta = [PacketMetadata::EMPTY; 8];
    let mut tx_meta = [PacketMetadata::EMPTY; 8];
    let mut buf = [0; 4096];

    let local_endpoint = IpEndpoint::new(
        embassy_net::IpAddress::Ipv4(Ipv4Address::new(192, 168, 7, 1)),
        1234,
    );

    let mut socket = UdpSocket::new(
        stack,
        &mut rx_meta,
        &mut rx_buffer,
        &mut tx_meta,
        &mut tx_buffer,
    );

    log::info!("Listening on UDP:1234...");
    match socket.bind(local_endpoint) {
        Ok(()) => {
            log::info!("bound to {}", local_endpoint);
        }
        Err(e) => {
            log::info!("bind {:?}", e);
        }
    }
    log::info!("local local_endpoint {:?}", socket.endpoint());

    loop {
        let (n, _meta) = match socket.recv_from(&mut buf).await {
            Ok((0, _)) => {
                log::info!("read EOF");
                continue;
            }
            Ok((n, meta)) => {
                log::info!("connection from {:?}", meta.endpoint);
                log::info!("reading {}", n);
                (n, meta)
            }
            Err(e) => {
                log::info!("read error: {:?}", e);
                continue;
            }
        };

        for (i, x) in buf[..n].iter().enumerate() {
            log::info!("rxd {} {}", i, x);
        }

        log::info!("decoding message");
        let msg = match from_bytes::<Command>(&buf[..]) {
            Ok(h) => {
                log::info!("message {:?}", h);
                h
            }
            Err(x) => {
                log::info!("invalid message ignored {}", x);
                continue;
            }
        };

        match msg {
            Command::SetDO { pin, value } => {
                log::info!("setting pin {} to {}", pin, value);
                send_to_channel(&DO_CHANNEL, DigitalOutMessages::Set { pin, value }).await;
            }
            _ => {
                log::info!("ignoring message {:?}", msg);
            }
        }
    }
}

#[embassy_executor::task]
async fn udp_output_task(
    stack: NetStack<'static>,
    mut watch: watch::Receiver<'static, CriticalSectionRawMutex, (), 32>,
) {
    watch.changed().await;

    let mut rx_buffer = [0; 4096];
    let mut tx_buffer = [0; 4096];
    let mut rx_meta = [PacketMetadata::EMPTY; 8];
    let mut tx_meta = [PacketMetadata::EMPTY; 8];

    let local_endpoint = IpEndpoint::new(
        embassy_net::IpAddress::Ipv4(Ipv4Address::new(192, 168, 7, 1)),
        1234,
    );

    let mut socket = UdpSocket::new(
        stack,
        &mut rx_meta,
        &mut rx_buffer,
        &mut tx_meta,
        &mut tx_buffer,
    );

    match socket.bind(local_endpoint) {
        Ok(()) => {
            log::info!("bound to {}", local_endpoint);
        }
        Err(e) => {
            log::info!("bind {:?}", e);
        }
    }
    log::info!("local local_endpoint {:?}", socket.endpoint());

    loop {
        let (endpoint, msg, diag) = NETWORK_OUT.receive().await;

        log::info!("recv echo");
        let mut buf = [0_u8; 256];
        match to_slice(&(msg, diag), &mut buf) {
            Ok(x) => {
                match socket.send_to(x, endpoint).await {
                    Ok(()) => {
                        log::info!("sent to network");
                    }
                    Err(e) => {
                        log::info!("write error: {:?}", e);
                    }
                };
            }
            Err(e) => {
                log::info!("encoding failed {}", e);
            }
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
enum DigitalOutMessages {
    Set { pin: u8, value: bool },
}

static DO_CHANNEL: Channel<CriticalSectionRawMutex, DigitalOutMessages, 16> = Channel::new();

async fn send_to_channel<T>(channel: &Channel<CriticalSectionRawMutex, T, 16>, value: T) {
    channel.send(value).await;
}

#[embassy_executor::task]
async fn digital_output_task(
    mut watch: watch::Receiver<'static, CriticalSectionRawMutex, (), 32>,
    p: [(u8, Output<'static>); 1],
) {
    watch.changed().await;

    let remote_endpoint = IpEndpoint::new(
        embassy_net::IpAddress::Ipv4(Ipv4Address::new(192, 168, 64, 47)),
        12345,
    );

    let mut pins: LinearMap<_, _, 1> = LinearMap::new();
    for (k, v) in p {
        pins.insert(k, v).unwrap();
    }

    let mut seconds = Ticker::every(Duration::from_secs(1));

    let mut start_time = Instant::now();
    loop {
        let end_time = Instant::now();
        let diff_time = end_time
            .checked_duration_since(start_time)
            .unwrap_or(Duration::from_secs(0));

        let msg = match select(seconds.next(), DO_CHANNEL.receiver().receive()).await {
            Either::First(()) => None,
            Either::Second(msg) => Some(msg),
        };

        let jitter = match start_time.checked_add(Duration::from_secs(1)) {
            None => Duration::from_secs(0),
            Some(expected) => Instant::now()
                .checked_duration_since(expected)
                .unwrap_or(Duration::from_secs(0)),
        };
        start_time = Instant::now();

        log::info!("digital out 0");
        match msg {
            None => {
                log::info!(
                    "times: {} {} {} {}",
                    diff_time.as_micros(),
                    jitter.as_micros(),
                    start_time.as_millis(),
                    end_time.as_millis()
                );
                let pins: Vec<_, 1> = pins
                    .iter()
                    .map(|(n, pin)| {
                        (
                            *n,
                            match pin.get_output_level() {
                                Level::Low => false,
                                Level::High => true,
                            },
                        )
                    })
                    .collect();
                let pins = *pins.as_array().unwrap();
                let header = Notification::DO { pins };
                let diagnostics = Diagnostics {
                    timestamp_us: Instant::now().as_micros(),
                    execution_us: diff_time.as_micros(),
                    jitter_in_us: jitter.as_micros(),
                    period_in_us: Duration::from_secs(1).as_micros(),
                };

                log::info!("digital out 5");
                NETWORK_OUT
                    .send((remote_endpoint, header, diagnostics))
                    .await;
                log::info!("digital out 6");
            }
            Some(DigitalOutMessages::Set { pin, value }) => {
                log::info!("digital out 1");
                match pins.get_mut(&pin) {
                    None => log::info!("unknown pin {}", 25),
                    Some(pin) => {
                        pin.set_level(match value {
                            false => Level::Low,
                            true => Level::High,
                        });
                    }
                }
                log::info!("digital out 2");
            }
        }

        ALIVE.signal(());
    }
}
