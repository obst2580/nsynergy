use anyhow::{Context, Result};
use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};
use std::collections::HashMap;
use std::net::Ipv4Addr;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

/// mDNS service type for nsynergy.
pub const SERVICE_TYPE: &str = "_nsynergy._udp.local.";

/// Information about a discovered peer on the LAN.
#[derive(Debug, Clone, PartialEq)]
pub struct PeerInfo {
    pub name: String,
    pub address: Ipv4Addr,
    pub udp_port: u16,
    pub tcp_port: u16,
}

/// Events emitted by the discovery system.
#[derive(Debug, Clone)]
pub enum DiscoveryEvent {
    PeerFound(PeerInfo),
    PeerLost(String),
}

/// Registers this machine as an nsynergy service on the LAN.
pub struct ServiceRegistration {
    daemon: ServiceDaemon,
    fullname: String,
}

impl ServiceRegistration {
    /// Registers a new service instance.
    ///
    /// - `machine_name`: human-readable name for this machine
    /// - `udp_port`: the UDP port for input events
    /// - `tcp_port`: the TCP port for clipboard data
    pub fn register(machine_name: &str, udp_port: u16, tcp_port: u16) -> Result<Self> {
        let daemon = ServiceDaemon::new().context("creating mDNS daemon")?;

        let host_name = format!("{machine_name}.local.");
        let mut properties = HashMap::new();
        properties.insert("tcp_port".to_string(), tcp_port.to_string());

        let service_info = ServiceInfo::new(
            SERVICE_TYPE,
            machine_name,
            &host_name,
            "",
            udp_port,
            properties,
        )
        .context("creating ServiceInfo")?;

        let fullname = service_info.get_fullname().to_string();
        daemon
            .register(service_info)
            .context("registering mDNS service")?;

        info!(
            name = machine_name,
            udp_port, tcp_port, "mDNS service registered"
        );

        Ok(Self { daemon, fullname })
    }

    /// Unregisters the service.
    pub fn unregister(self) -> Result<()> {
        self.daemon
            .unregister(&self.fullname)
            .context("unregistering mDNS service")?;
        debug!(fullname = %self.fullname, "mDNS service unregistered");
        Ok(())
    }
}

/// Browses the LAN for nsynergy peers and sends discovery events.
pub struct ServiceBrowser {
    daemon: ServiceDaemon,
}

impl ServiceBrowser {
    /// Starts browsing for nsynergy services.
    /// Returns a channel receiver that emits `DiscoveryEvent`s.
    pub fn start() -> Result<(Self, mpsc::UnboundedReceiver<DiscoveryEvent>)> {
        let daemon = ServiceDaemon::new().context("creating mDNS browse daemon")?;
        let receiver = daemon
            .browse(SERVICE_TYPE)
            .context("starting mDNS browse")?;

        let (tx, rx) = mpsc::unbounded_channel();

        // Spawn a blocking thread to read from the mdns-sd sync channel
        // and forward events to our async mpsc channel.
        std::thread::spawn(move || {
            while let Ok(event) = receiver.recv() {
                match event {
                    ServiceEvent::ServiceResolved(info) => {
                        let name = info.get_fullname().to_string();
                        let addresses = info.get_addresses();

                        let ipv4 = addresses.iter().find_map(|addr| {
                            if let std::net::IpAddr::V4(v4) = addr {
                                Some(*v4)
                            } else {
                                None
                            }
                        });

                        let Some(address) = ipv4 else {
                            warn!(name, "resolved service has no IPv4 address, skipping");
                            continue;
                        };

                        let tcp_port = info
                            .get_property_val_str("tcp_port")
                            .and_then(|s| s.parse::<u16>().ok())
                            .unwrap_or(24801);

                        let peer = PeerInfo {
                            name: name.clone(),
                            address,
                            udp_port: info.get_port(),
                            tcp_port,
                        };

                        debug!(?peer, "peer discovered");
                        if tx.send(DiscoveryEvent::PeerFound(peer)).is_err() {
                            break;
                        }
                    }
                    ServiceEvent::ServiceRemoved(_, fullname) => {
                        debug!(fullname, "peer lost");
                        if tx
                            .send(DiscoveryEvent::PeerLost(fullname))
                            .is_err()
                        {
                            break;
                        }
                    }
                    _ => {}
                }
            }
        });

        info!("mDNS browser started");
        Ok((Self { daemon }, rx))
    }

    /// Shuts down the browser daemon.
    pub fn shutdown(self) -> Result<()> {
        self.daemon.shutdown().context("shutting down mDNS browser")?;
        Ok(())
    }
}

/// Tracks known peers in a thread-safe map.
#[derive(Debug, Clone, Default)]
pub struct PeerRegistry {
    peers: Arc<Mutex<HashMap<String, PeerInfo>>>,
}

impl PeerRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&self, peer: PeerInfo) {
        let mut peers = self.peers.lock().expect("lock poisoned");
        peers.insert(peer.name.clone(), peer);
    }

    pub fn remove(&self, name: &str) {
        let mut peers = self.peers.lock().expect("lock poisoned");
        peers.remove(name);
    }

    pub fn list(&self) -> Vec<PeerInfo> {
        let peers = self.peers.lock().expect("lock poisoned");
        peers.values().cloned().collect()
    }

    pub fn get(&self, name: &str) -> Option<PeerInfo> {
        let peers = self.peers.lock().expect("lock poisoned");
        peers.get(name).cloned()
    }

    pub fn count(&self) -> usize {
        let peers = self.peers.lock().expect("lock poisoned");
        peers.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn peer_registry_add_remove() {
        let registry = PeerRegistry::new();
        assert_eq!(registry.count(), 0);

        let peer = PeerInfo {
            name: "test-machine._nsynergy._udp.local.".to_string(),
            address: Ipv4Addr::new(192, 168, 1, 10),
            udp_port: 24800,
            tcp_port: 24801,
        };

        registry.add(peer.clone());
        assert_eq!(registry.count(), 1);
        assert_eq!(registry.get(&peer.name), Some(peer.clone()));

        registry.remove(&peer.name);
        assert_eq!(registry.count(), 0);
        assert_eq!(registry.get(&peer.name), None);
    }

    #[test]
    fn peer_registry_list() {
        let registry = PeerRegistry::new();

        let p1 = PeerInfo {
            name: "machine-a".to_string(),
            address: Ipv4Addr::new(192, 168, 1, 10),
            udp_port: 24800,
            tcp_port: 24801,
        };
        let p2 = PeerInfo {
            name: "machine-b".to_string(),
            address: Ipv4Addr::new(192, 168, 1, 20),
            udp_port: 24800,
            tcp_port: 24801,
        };

        registry.add(p1.clone());
        registry.add(p2.clone());

        let list = registry.list();
        assert_eq!(list.len(), 2);
        assert!(list.contains(&p1));
        assert!(list.contains(&p2));
    }

    #[test]
    fn peer_registry_overwrite() {
        let registry = PeerRegistry::new();

        let peer_v1 = PeerInfo {
            name: "machine".to_string(),
            address: Ipv4Addr::new(192, 168, 1, 10),
            udp_port: 24800,
            tcp_port: 24801,
        };
        let peer_v2 = PeerInfo {
            name: "machine".to_string(),
            address: Ipv4Addr::new(192, 168, 1, 20),
            udp_port: 25000,
            tcp_port: 25001,
        };

        registry.add(peer_v1);
        registry.add(peer_v2.clone());

        assert_eq!(registry.count(), 1);
        let got = registry.get("machine").unwrap();
        assert_eq!(got.address, Ipv4Addr::new(192, 168, 1, 20));
        assert_eq!(got.udp_port, 25000);
    }

    #[test]
    fn service_type_format() {
        assert!(SERVICE_TYPE.starts_with('_'));
        assert!(SERVICE_TYPE.contains("._udp."));
        assert!(SERVICE_TYPE.ends_with('.'));
    }
}
