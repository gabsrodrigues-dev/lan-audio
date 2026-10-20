use std::collections::BTreeSet;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::Context;
use if_addrs::{get_if_addrs, IfAddr};
use lan_audio_protocol::DiscoveryBeacon;
use lan_audio_protocol::{REVERSE_CONTROL_PORT, REVERSE_TCP_PORT};
use mdns_sd::{ServiceDaemon, ServiceInfo};
use tokio::net::UdpSocket;
use tokio::sync::broadcast;
use tracing::{info, warn};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct DiscoveryConfig {
    pub server_id: Uuid,
    pub server_name: String,
    pub bind_addr: SocketAddr,
    pub broadcast_addr: SocketAddr,
    pub ws_port: u16,
    pub udp_port: u16,
}

#[derive(Debug, Clone)]
pub struct MdnsServiceConfig {
    pub server_name: String,
    pub ws_port: u16,
    pub version: String,
    pub mode: String,
}

pub async fn run_discovery_broadcast(
    cfg: DiscoveryConfig,
    mut shutdown: broadcast::Receiver<()>,
) -> anyhow::Result<()> {
    let socket = UdpSocket::bind(cfg.bind_addr)
        .await
        .with_context(|| format!("bind discovery socket: {}", cfg.bind_addr))?;
    socket.set_broadcast(true)?;
    let targets = discovery_targets(cfg.broadcast_addr);
    let bind = socket.local_addr().unwrap_or(cfg.bind_addr);
    let targets_text = targets
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(", ");

    info!(
        bind = %bind,
        broadcast = %cfg.broadcast_addr,
        targets = %targets_text,
        "discovery broadcaster started"
    );

    loop {
        tokio::select! {
            _ = shutdown.recv() => {
                info!("discovery broadcaster stopping");
                return Ok(());
            }
            _ = tokio::time::sleep(Duration::from_secs(1)) => {
                let beacon = DiscoveryBeacon {
                    kind: "lan_audio_discovery_v1".to_string(),
                    server_id: cfg.server_id,
                    server_name: cfg.server_name.clone(),
                    ws_port: cfg.ws_port,
                    udp_port: cfg.udp_port,
                    reverse_channel_port: REVERSE_TCP_PORT,
                    control_channel_port: REVERSE_CONTROL_PORT,
                    ts_unix_ms: now_ms(),
                };
                match serde_json::to_vec(&beacon) {
                    Ok(payload) => {
                        for target in &targets {
                            if let Err(err) = socket.send_to(&payload, target).await {
                                warn!(error = %err, target = %target, "failed sending discovery beacon");
                            }
                        }
                    }
                    Err(err) => warn!(error = %err, "failed serialize discovery beacon"),
                }
            }
        }
    }
}

pub async fn run_mdns_registration(
    cfg: MdnsServiceConfig,
    mut shutdown: broadcast::Receiver<()>,
) -> anyhow::Result<()> {
    let service_type = "_lan-audio._tcp.local.";
    let instance_name = mdns_instance_name(&cfg.server_name);
    let host_name = mdns_host_name(&cfg.server_name);
    let host_ips = local_ipv4_addrs();

    if host_ips.is_empty() {
        warn!("mDNS registration skipped: no non-loopback IPv4 address");
        let _ = shutdown.recv().await;
        return Ok(());
    }

    let txt = [
        ("version".to_string(), cfg.version.clone()),
        ("mode".to_string(), cfg.mode.clone()),
    ];
    let service = ServiceInfo::new(
        service_type,
        &instance_name,
        &host_name,
        host_ips.as_slice(),
        cfg.ws_port,
        txt.as_slice(),
    )
    .context("create mDNS service info")?;
    let daemon = ServiceDaemon::new().context("create mDNS service daemon")?;
    daemon
        .register(service)
        .context("register LAN Audio mDNS service")?;

    info!(
        service_type,
        instance = %instance_name,
        host = %host_name,
        port = cfg.ws_port,
        "mDNS service registered"
    );

    let _ = shutdown.recv().await;
    info!("mDNS service stopping");
    daemon.shutdown().context("shutdown mDNS service daemon")?;
    Ok(())
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub type SharedDiscoveryConfig = Arc<DiscoveryConfig>;

/// LAN endpoints must not use VPN or container addresses for phone connections.
pub fn local_ipv4_addrs() -> Vec<IpAddr> {
    let mut interfaces = get_if_addrs().unwrap_or_default();
    interfaces.retain(|iface| {
        matches!(&iface.addr, IfAddr::V4(v4) if !v4.ip.is_loopback() && !v4.ip.is_unspecified())
            && is_lan_interface_name(&iface.name)
            && interface_is_active(&iface.name)
    });
    // Hardware adapters precede software bridges and VPN-routed defaults.
    interfaces.sort_by_key(|iface| !is_hardware_interface(&iface.name));
    interfaces.into_iter().map(|iface| iface.ip()).collect()
}

fn is_lan_interface_name(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    ![
        "docker",
        "veth",
        "br-",
        "virbr",
        "mpqemubr",
        "tun",
        "tap",
        "wg",
        "utun",
        "tailscale",
        "vmnet",
        "vboxnet",
    ]
    .iter()
    .any(|prefix| name.starts_with(prefix))
        && !name.contains("warp")
        && !name.contains("vpn")
}

fn interface_is_active(name: &str) -> bool {
    #[cfg(target_os = "linux")]
    {
        return std::fs::read_to_string(format!("/sys/class/net/{name}/operstate"))
            .map(|state| state.trim() != "down")
            .unwrap_or(true);
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = name;
        true
    }
}

fn is_hardware_interface(name: &str) -> bool {
    #[cfg(target_os = "linux")]
    {
        return std::path::Path::new("/sys/class/net")
            .join(name)
            .join("device")
            .exists();
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = name;
        true
    }
}

pub fn preferred_lan_ip() -> Option<IpAddr> {
    let addresses = local_ipv4_addrs();
    // Keep normal OS routing preferences when the routed endpoint is on LAN.
    let routed = std::net::UdpSocket::bind("0.0.0.0:0")
        .ok()
        .and_then(|socket| {
            socket.connect("8.8.8.8:80").ok()?;
            Some(socket.local_addr().ok()?.ip())
        });
    routed
        .filter(|ip| addresses.contains(ip))
        .or_else(|| addresses.into_iter().next())
}

fn mdns_instance_name(server_name: &str) -> String {
    let clean = server_name.trim();
    if clean.is_empty() {
        "LAN Audio".to_string()
    } else if clean.starts_with("LAN Audio @") {
        clean.to_string()
    } else {
        format!("LAN Audio @ {clean}")
    }
}

fn mdns_host_name(server_name: &str) -> String {
    let mut host = server_name
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>();
    while host.contains("--") {
        host = host.replace("--", "-");
    }
    let host = host.trim_matches('-');
    let host = if host.is_empty() { "lan-audio" } else { host };
    format!("{host}.local.")
}

fn discovery_targets(default_target: SocketAddr) -> Vec<SocketAddr> {
    let mut targets = BTreeSet::new();
    targets.insert(default_target);

    if let Ok(interfaces) = get_if_addrs() {
        for iface in interfaces {
            let IfAddr::V4(v4) = iface.addr else {
                continue;
            };
            if v4.ip.is_loopback() {
                continue;
            }
            if !is_lan_interface_name(&iface.name) || !interface_is_active(&iface.name) {
                continue;
            }
            let broadcast = v4
                .broadcast
                .unwrap_or_else(|| directed_broadcast(v4.ip, v4.netmask));
            targets.insert(SocketAddr::new(
                IpAddr::V4(broadcast),
                default_target.port(),
            ));
        }
    }

    targets.into_iter().collect()
}

fn directed_broadcast(ip: Ipv4Addr, netmask: Ipv4Addr) -> Ipv4Addr {
    Ipv4Addr::from(u32::from(ip) | !u32::from(netmask))
}

#[cfg(test)]
mod tests {
    use super::{directed_broadcast, mdns_host_name, mdns_instance_name};
    use std::net::Ipv4Addr;

    #[test]
    fn phone_endpoints_exclude_vpn_and_container_interfaces() {
        for name in [
            "CloudflareWARP",
            "docker0",
            "docker_gwbridge",
            "br-a123",
            "veth123",
            "mpqemubrzone1",
            "tun0",
            "tailscale0",
        ] {
            assert!(!super::is_lan_interface_name(name), "{name}");
        }
        for name in ["eno1", "wlan0", "enp4s0", "Wi-Fi", "Ethernet", "br0"] {
            assert!(super::is_lan_interface_name(name), "{name}");
        }
    }

    #[test]
    fn directed_broadcast_uses_ip_and_mask() {
        assert_eq!(
            directed_broadcast(
                Ipv4Addr::new(192, 168, 31, 44),
                Ipv4Addr::new(255, 255, 255, 0)
            ),
            Ipv4Addr::new(192, 168, 31, 255)
        );
        assert_eq!(
            directed_broadcast(Ipv4Addr::new(10, 0, 0, 18), Ipv4Addr::new(255, 255, 0, 0)),
            Ipv4Addr::new(10, 0, 255, 255)
        );
    }

    #[test]
    fn mdns_names_are_stable_and_readable() {
        assert_eq!(
            mdns_instance_name("Office PC"),
            "LAN Audio @ Office PC".to_string()
        );
        assert_eq!(mdns_host_name("Office PC"), "office-pc.local.".to_string());
        assert_eq!(mdns_host_name(""), "lan-audio.local.".to_string());
    }
}
