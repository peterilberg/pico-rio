//! https://github.com/embassy-rs/embassy/blob/main/examples/rp/src/bin/usb_ethernet.rs
//!
//! This example shows how to use USB (Universal Serial Bus) in the RP2040 chip.
//!
//! This is a CDC-NCM class implementation, aka Ethernet over USB.

#![no_std]
#![no_main]

use assign_resources::assign_resources;
use core::cmp::{max, min};
use defmt::*;
use embassy_executor::Executor;
use embassy_executor::Spawner;
use embassy_futures::select::{Either, Either3, select, select3};
use embassy_net::tcp::TcpSocket;
use embassy_net::udp::{PacketMetadata, UdpMetadata, UdpSocket};
use embassy_net::{
    IpEndpoint, Ipv4Address, Ipv4Cidr, Stack as NetStack, StackResources, StaticConfigV4,
};
use embassy_rp::Peri;
use embassy_rp::Peripherals;
use embassy_rp::clocks::RoscRng;
use embassy_rp::gpio::{Level, Output};
use embassy_rp::multicore::{Stack, spawn_core1};
use embassy_rp::peripherals::USB;
use embassy_rp::usb::{Driver, InterruptHandler};
use embassy_rp::{bind_interrupts, peripherals};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
// use embassy_sync::channel::Channel;
use embassy_rp::watchdog::Watchdog;
use embassy_sync::mutex::Mutex;
use embassy_sync::signal;
use embassy_time::Duration;
use embassy_time::Instant;
use embassy_time::Ticker;
use embassy_time::Timer;
use embassy_usb::class::cdc_acm::{CdcAcmClass, State as AcmState};
use embassy_usb::class::cdc_ncm::embassy_net::{Device, Runner, State as NetState};
use embassy_usb::class::cdc_ncm::{CdcNcmClass, State as NcmState};
use embassy_usb::{Builder, Config, UsbDevice};
// use embedded_io_async::Write;
use heapless::Vec;
use leasehund::{
    DHCPServerBuffers, DHCPServerSocket, DhcpConfig as LeaseConfig, DhcpConfigBuilder, DhcpServer,
    TransactionEvent,
};
use postcard::{from_bytes, to_slice};
use serde::{Deserialize, Serialize};
use static_cell::StaticCell;
use {defmt_rtt as _, panic_probe as _};

use embassy_sync::zerocopy_channel::{Channel as ZeroCopyChannel, Receiver, Sender};

// static CHANNEL: Channel<CriticalSectionRawMutex, LedState, 1> = Channel::new();

static STATE_CHANGED: signal::Signal<CriticalSectionRawMutex, ()> = signal::Signal::new();
static ALIVE: signal::Signal<CriticalSectionRawMutex, ()> = signal::Signal::new();

const PACKET_SIZE: usize = 256;
const NUM_PACKETS: usize = 8;
type Packet = (UdpMetadata, [u8; PACKET_SIZE]);

bind_interrupts!(struct Irqs {
    USBCTRL_IRQ => InterruptHandler<USB>;
});

assign_resources! {
    watchdog: WatchdogResources {
        watchdog: WATCHDOG,
    }
    usb: UsbResources{
        usb: USB,
    }
    digitalOut: DigitalOutResources{
        pin_25: PIN_25,
    }
}

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
    let r = split_resources!(p);

    let null_endpoint = IpEndpoint::new(
        embassy_net::IpAddress::Ipv4(Ipv4Address::new(0, 0, 0, 0)),
        0,
    );

    let meta = UdpMetadata::from(null_endpoint);

    static BUF0: StaticCell<[Packet; NUM_PACKETS]> = StaticCell::new();
    let buf0 = BUF0.init([(meta, [0; PACKET_SIZE]); NUM_PACKETS]);

    static BUF1: StaticCell<[Packet; NUM_PACKETS]> = StaticCell::new();
    let buf1 = BUF1.init([(meta, [0; PACKET_SIZE]); NUM_PACKETS]);

    static CHANNEL_0: StaticCell<ZeroCopyChannel<'_, CriticalSectionRawMutex, Packet>> =
        StaticCell::new();
    let channel0 = CHANNEL_0.init(ZeroCopyChannel::new(buf0));
    let (sender0, receiver0) = channel0.split();

    static CHANNEL_1: StaticCell<ZeroCopyChannel<'_, CriticalSectionRawMutex, Packet>> =
        StaticCell::new();
    let channel1 = CHANNEL_1.init(ZeroCopyChannel::new(buf1));
    let (sender1, receiver1) = channel1.split();

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
                spawner.spawn(unwrap!(core1_task(
                    spawner,
                    r.digitalOut,
                    sender0,
                    receiver1
                )))
            });
        },
    );

    let executor0 = EXECUTOR0.init(Executor::new());
    executor0.run(|spawner| {
        spawner.spawn(unwrap!(core0_task(
            spawner, r.usb, r.watchdog, sender1, receiver0
        )));
    });
}

#[embassy_executor::task]
async fn core0_task(
    spawner: Spawner,
    usb: UsbResources,
    watchdog: WatchdogResources,
    mut sender: Sender<'static, CriticalSectionRawMutex, Packet>,
    mut receiver: Receiver<'static, CriticalSectionRawMutex, Packet>,
) {
    let mut rng = RoscRng;

    let mut watchdog = Watchdog::new(watchdog.watchdog);

    // Create the driver, from the HAL.
    let driver = Driver::new(usb.usb, Irqs);

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
    let mut builder = Builder::new(
        driver,
        config,
        &mut CONFIG_DESC.init([0; 256])[..],
        &mut BOS_DESC.init([0; 256])[..],
        &mut [], // no msos descriptors
        &mut CONTROL_BUF.init([0; 128])[..],
    );

    // Our MAC addr.
    let our_mac_addr = [0xCC, 0xCC, 0xCC, 0xCC, 0xCC, 0xCC];
    // Host's MAC addr. This is the MAC the host "thinks" its USB-to-ethernet adapter has.
    let host_mac_addr = [0x88, 0x88, 0x88, 0x88, 0x88, 0x88];

    // Create classes on the builder.
    static STATE: StaticCell<NcmState> = StaticCell::new();
    let class = CdcNcmClass::new(&mut builder, STATE.init(NcmState::new()), host_mac_addr, 64);

    // Create a class for the logger
    static LOG_STATE: StaticCell<AcmState> = StaticCell::new();
    let logger_class = CdcAcmClass::new(&mut builder, LOG_STATE.init(AcmState::new()), 64);

    // Build the builder.
    let usb = builder.build();

    spawner.spawn(unwrap!(usb_task(usb)));
    spawner.spawn(unwrap!(log_task(logger_class)));

    static NET_STATE: StaticCell<NetState<MTU, 4, 4>> = StaticCell::new();
    let (runner, device) =
        class.into_embassy_net_device::<MTU, 4, 4>(NET_STATE.init(NetState::new()), our_mac_addr);
    spawner.spawn(unwrap!(usb_ncm_task(runner)));

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
    let seed = rng.next_u64();

    // Init network stack
    static RESOURCES: StaticCell<StackResources<10>> = StaticCell::new();
    let (stack, runner) =
        embassy_net::new(device, config, RESOURCES.init(StackResources::new()), seed);
    log::info!("waiting for config up");
    stack.wait_config_up().await;

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

    let dhcp_server: DhcpServer<32, 4> = DhcpServer::with_config(config);
    spawner.spawn(unwrap!(dhcp_task(dhcp_server, stack)));
    spawner.spawn(unwrap!(net_task(runner)));

    // And now we can use it!
    STATE_CHANGED.signal(());

    let mut rx_buffer = [0; 4096];
    let mut tx_buffer = [0; 4096];
    let mut rx_meta = [PacketMetadata::EMPTY; 8];
    let mut tx_meta = [PacketMetadata::EMPTY; 8];
    let mut buf = [0; 4096];

    let mut udp_rx_buffer = [0; 4096];
    let mut udp_tx_buffer = [0; 4096];
    let mut udp_rx_meta = [PacketMetadata::EMPTY; 8];
    let mut udp_tx_meta = [PacketMetadata::EMPTY; 8];

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

    watchdog.start(Duration::from_secs(3));

    loop {
        /*
                    let n = match socket.read(&mut buf).await {
                        Ok(0) => {
                            log::info!("read EOF");
                            break;
                        }
                        Ok(n) => {
                            log::info!("reading {}", n);
                            n
                        }
                        Err(e) => {
                            log::info!("read error: {:?}", e);
                            break;
                        }
                    };
        */
        let (n, meta) =
            match select3(socket.recv_from(&mut buf), receiver.receive(), ALIVE.wait()).await {
                Either3::First(Ok((0, _))) => {
                    log::info!("read EOF");
                    watchdog.feed(Duration::from_secs(3));
                    continue;
                }
                Either3::First(Ok((n, meta))) => {
                    log::info!("connection from {:?}", meta.endpoint);
                    log::info!("reading {}", n);
                    watchdog.feed(Duration::from_secs(3));
                    (n, meta)
                }
                Either3::First(Err(e)) => {
                    log::info!("read error: {:?}", e);
                    watchdog.feed(Duration::from_secs(3));
                    continue;
                }
                Either3::Second(buf) => {
                    log::info!("recv echo");
                    match socket.send_to(&buf.1, buf.0.endpoint).await {
                        Ok(()) => {}
                        Err(e) => {
                            log::info!("write error: {:?}", e);
                        }
                    };
                    receiver.receive_done();
                    log::info!("continuing");
                    watchdog.feed(Duration::from_secs(3));
                    continue;
                }
                Either3::Third(_) => {
                    log::info!("nothing happened");
                    watchdog.feed(Duration::from_secs(3));
                    continue;
                }
            };

        for (i, x) in buf[..n].iter().enumerate() {
            log::info!("rxd {} {}", i, x);
        }

        log::info!("sending packet to core 1");
        let packet = sender.send().await;
        let len = min(n, packet.1.len());
        let (left, right) = packet.1.split_at_mut(len);
        left.copy_from_slice(&buf[..len]);
        right.fill(0);
        packet.0 = meta;
        sender.send_done();
        log::info!("sent");

        /*

        match socket.write_all(&buf[..n]).await {
            Ok(()) => {}
            Err(e) => {
                log::info!("write error: {:?}", e);
                break;
            }
        };
        */

        let mut udp = UdpSocket::new(
            stack,
            &mut udp_rx_meta,
            &mut udp_rx_buffer,
            &mut udp_tx_meta,
            &mut udp_tx_buffer,
        );
        // let local_endpoint = IpEndpoint::new(embassy_net::IpAddress::Ipv4(Ipv4Address::UNSPECIFIED), 12345);
        match udp.bind(local_endpoint) {
            Ok(()) => {
                log::info!("bound to {}", local_endpoint);
            }
            Err(e) => {
                log::info!("bind {:?}", e);
            }
        }
        let remote_endpoint = IpEndpoint::new(
            embassy_net::IpAddress::Ipv4(Ipv4Address::new(192, 168, 64, 47)),
            12345,
        );

        let mut status = [0_u8; PACKET_SIZE];
        let msg = Message {
            sender: 123,
            time_ms: Instant::now().as_millis(),
            kind: Kind::Status,
        };
        let result = to_slice(&msg, &mut status).unwrap();
        match udp.send_to(&result, remote_endpoint).await {
            Ok(()) => {
                log::info!("sent to {}", remote_endpoint);
            }
            Err(e) => {
                log::info!("send_to {:?}", e)
            }
        }
        udp.flush().await;
        log::info!("flushed udp socket");
    }
}

enum LedState {
    On,
    Off,
}

#[embassy_executor::task]
async fn core1_task(
    _spawner: Spawner,
    p: DigitalOutResources,
    mut sender: Sender<'static, CriticalSectionRawMutex, Packet>,
    mut receiver: Receiver<'static, CriticalSectionRawMutex, Packet>,
) {
    let mut led = Output::new(p.pin_25, Level::Low);
    STATE_CHANGED.wait().await;

    let mut seconds = Ticker::every(Duration::from_secs(1));
    loop {
        match select(seconds.next(), receiver.receive()).await {
            Either::First(_) => {
                log::info!("toggle");
                led.toggle();
                ALIVE.signal(());
            }

            Either::Second((meta0, buf)) => {
                let now = Instant::now();
                let milli = now.as_millis();
                let mut buf2 = [0_u8; 32];
                let len = buf2.len();
                buf2.copy_from_slice(&buf[..len]);
                let header = Message {
                    sender: 123,
                    time_ms: milli,
                    kind: Kind::Echo(buf2),
                };
                log::info!("echo0");
                let (meta, out) = sender.send().await;
                log::info!("echo1");

                *meta = *meta0;

                let _ = to_slice(&header, out).unwrap();
                log::info!("echo2");

                let y = from_bytes::<Message>(out);
                match y {
                    Ok(h) => {
                        log::info!("header {:?}", h);
                    }
                    Err(x) => {
                        log::info!("header err {}", x);
                    }
                }
                sender.send_done();
                log::info!("echo3");
                receiver.receive_done();
                log::info!("echo4");
            }
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
enum Kind {
    Status,
    Echo([u8; 32]),
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
struct Message {
    sender: u32,
    time_ms: u64,
    kind: Kind,
}
