use embassy_net::udp::{PacketMetadata, UdpSocket};
use embassy_net::{Config, ConfigV4, Runner, Stack, StackResources, StaticConfigV4};
use embassy_net::{IpAddress, IpEndpoint, Ipv4Address, Ipv4Cidr};
use embassy_rp::clocks::RoscRng;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::watch::Watch;
use heapless::Vec;
use leasehund::{DHCPServerBuffers, DHCPServerSocket, TransactionEvent};
use leasehund::{DhcpConfig, DhcpConfigBuilder, DhcpServer};
use static_cell::StaticCell;
use {defmt_rtt as _, panic_probe as _};

use crate::usb::NetworkCard;

const NUM_SOCKETS: usize = 32;
static NETWORK_READY: Watch<CriticalSectionRawMutex, (), NUM_SOCKETS> = Watch::new();

#[derive(Clone, Copy)]
pub struct NetworkStack(Stack<'static>);
pub type Network = Runner<'static, NetworkCard>;

pub fn new_network(
    network_card: NetworkCard,
    configuration: StaticConfigV4,
) -> (Network, NetworkStack) {
    let config = Config::ipv4_static(configuration);

    let mut rng = RoscRng;
    let random_seed = rng.next_u64();

    static RESOURCES: StaticCell<StackResources<NUM_SOCKETS>> = StaticCell::new();
    let resources = RESOURCES.init(StackResources::new());
    let (stack, driver) = embassy_net::new(network_card, config, resources, random_seed);
    (driver, NetworkStack(stack))
}

#[embassy_executor::task]
pub async fn network_task(mut task: Network) -> ! {
    task.run().await
}

impl NetworkStack {
    pub fn add_dhcp_server(&self) -> DhcpServer<32, 4> {
        let NetworkStack(stack) = self;

        let default_ip = Ipv4Address::new(0, 0, 0, 0);
        let default_config = StaticConfigV4 {
            address: Ipv4Cidr::new(default_ip, 24),
            dns_servers: Vec::new(),
            gateway: Some(default_ip),
        };
        let config_v4 = stack.config_v4().unwrap_or(default_config);

        let ip = config_v4.address.address();
        let mask = config_v4.address.netmask();
        let router = config_v4.gateway.unwrap_or(default_ip);

        let cloudflare_dns = Ipv4Address::new(1, 1, 1, 1);
        let backup_dns = Ipv4Address::new(1, 0, 0, 1);
        let google_dns = Ipv4Address::new(8, 8, 8, 8);

        let [a, b, c, _] = config_v4.address.address().octets();
        let start = Ipv4Address::new(a, b, c, 100);
        let end = Ipv4Address::new(a, b, c, 200);

        let config: DhcpConfig<4> = DhcpConfigBuilder::new()
            .server_ip(ip)
            .subnet_mask(mask)
            .router(router)
            .add_dns_server(cloudflare_dns)
            .add_dns_server(backup_dns)
            .add_dns_server(google_dns)
            .ip_pool(start, end)
            .lease_time(7200) // 2 hours
            .build();

        DhcpServer::with_config(config)
    }

    pub fn new_udp_socket<'socket>(
        &self,
        port: u16,
        buffers: &'socket mut SocketBuffers,
    ) -> UdpSocket<'socket> {
        let NetworkStack(stack) = self;

        let SocketBuffers {
            rx_buffer,
            tx_buffer,
            rx_meta,
            tx_meta,
        } = buffers;
        let mut socket = UdpSocket::new(*stack, rx_meta, rx_buffer, tx_meta, tx_buffer);

        match stack.config_v4() {
            None => {}
            Some(config) => {
                let address = IpAddress::Ipv4(config.address.address());
                let endpoint = IpEndpoint::new(address, port);
                let _ = socket.bind(endpoint);
            }
        }

        socket
    }
}

pub struct SocketBuffers {
    rx_buffer: [u8; 4096],
    tx_buffer: [u8; 4096],
    rx_meta: [PacketMetadata; 8],
    tx_meta: [PacketMetadata; 8],
}

impl Default for SocketBuffers {
    fn default() -> Self {
        SocketBuffers {
            rx_buffer: [0; 4096],
            tx_buffer: [0; 4096],
            rx_meta: [PacketMetadata::EMPTY; 8],
            tx_meta: [PacketMetadata::EMPTY; 8],
        }
    }
}

#[embassy_executor::task]
pub async fn dhcp_task(mut server: DhcpServer<32, 4>, stack: NetworkStack) -> ! {
    let NetworkStack(stack) = stack;

    let mut buffers = DHCPServerBuffers::new();
    let mut socket = DHCPServerSocket::new(stack, &mut buffers);

    loop {
        let Ok(event) = server.lease_one(&mut socket).await else {
            continue; // Lease failed
        };

        match event {
            TransactionEvent::Leased(ip, _) => {
                let _ = stack.config_v4().map(|config| {
                    stack.set_config_v4(ConfigV4::Static(StaticConfigV4 {
                        gateway: Some(ip),
                        ..config
                    }));
                });
                stack.wait_config_up().await;
            }
            TransactionEvent::Released(_, _) => {}
        }
    }
}

#[embassy_executor::task]
pub async fn notify_when_available(stack: NetworkStack) {
    let NetworkStack(stack) = stack;
    stack.wait_config_up().await;
    NETWORK_READY.sender().send(());
}

pub async fn wait_for_network() {
    match NETWORK_READY.receiver() {
        None => {}
        Some(mut network) => network.changed().await,
    }
}
