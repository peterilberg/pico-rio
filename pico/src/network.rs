use embassy_net::{
    Ipv4Address, Runner as NetRunner, Stack as NetStack, StackResources, StaticConfigV4,
};
use embassy_rp::clocks::RoscRng;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::watch;
use embassy_usb::class::cdc_ncm::embassy_net::Device;

use leasehund::{
    DHCPServerBuffers, DHCPServerSocket, DhcpConfig as LeaseConfig, DhcpConfigBuilder, DhcpServer,
    TransactionEvent,
};
use static_cell::StaticCell;
use {defmt_rtt as _, panic_probe as _};

use embassy_sync::channel::Channel;

static STATE_CHANGED: watch::Watch<CriticalSectionRawMutex, (), 32> = watch::Watch::new();

const MTU: usize = 1514;

impl NDrive {
    pub async fn run(mut self) -> ! {
        self.0.run().await
    }
}

impl Dhcp {
    pub async fn run(self) -> ! {
        let Dhcp(mut server, NStack(stack)) = self;
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
}

type Mailbox<T> = Channel<CriticalSectionRawMutex, T, 16>;

pub struct NStack(pub NetStack<'static>);
pub struct NDrive(NetRunner<'static, Device<'static, MTU>>);

pub fn new_network_stack(
    network_card: Device<'static, MTU>,
    configuration: embassy_net::StaticConfigV4,
) -> (NStack, NDrive) {
    let config = embassy_net::Config::ipv4_static(configuration);

    // Generate random seed
    let mut rng = RoscRng;
    let seed = rng.next_u64();

    // Init network stack
    static RESOURCES: StaticCell<StackResources<10>> = StaticCell::new();
    let (s, r) = embassy_net::new(
        network_card,
        config,
        RESOURCES.init(StackResources::new()),
        seed,
    );
    (NStack(s), NDrive(r))
}

pub struct Dhcp(DhcpServer<32, 4>, NStack);

impl NStack {
    pub fn add_dhcp_server(&self) -> Dhcp {
        let v4 = self.0.config_v4().unwrap();
        let [a, b, c, _] = v4.address.address().octets();

        let config: LeaseConfig<4> = DhcpConfigBuilder::new()
            .server_ip(v4.address.address())
            .subnet_mask(v4.address.netmask())
            .router(v4.gateway.unwrap())
            .add_dns_server(Ipv4Address::new(1, 1, 1, 1)) // Cloudflare DNS
            .add_dns_server(Ipv4Address::new(1, 0, 0, 1)) // Cloudflare backup
            .add_dns_server(Ipv4Address::new(8, 8, 8, 8)) // Google DNS
            .ip_pool(
                Ipv4Address::new(a, b, c, 100),
                Ipv4Address::new(a, b, c, 200),
            )
            .lease_time(7200) // 2 hours
            .build();

        Dhcp(DhcpServer::with_config(config), NStack(self.0))
    }
}

#[embassy_executor::task]
pub async fn notify_when_network_is_available(stack: NStack) {
    log::info!("waiting for config up");
    stack.0.wait_config_up().await;

    // And now we can use it!
    STATE_CHANGED.sender().send(());
}

pub async fn wait_for_network() {
    match STATE_CHANGED.receiver() {
        None => {}
        Some(mut watch) => watch.changed().await,
    }
}

#[embassy_executor::task]
pub async fn net_task(task: NDrive) -> ! {
    task.run().await
}

#[embassy_executor::task]
pub async fn dhcp_task(task: Dhcp) -> ! {
    task.run().await;
}
