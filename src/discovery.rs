use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{mpsc, Mutex};

use crate::types::{DeviceConfig, DeviceType};
use crate::upnp;

pub enum DiscoveryEvent {
    DeviceFound(DeviceConfig),
    DeviceLost(String),
}

pub struct DeviceDiscovery {
    seen_ids: Arc<Mutex<HashSet<String>>>,
    handles: Vec<tokio::task::JoinHandle<()>>,
}

impl DeviceDiscovery {
    pub fn new() -> Self {
        Self {
            seen_ids: Arc::new(Mutex::new(HashSet::new())),
            handles: Vec::new(),
        }
    }

    pub fn start(&mut self, event_tx: mpsc::Sender<DiscoveryEvent>) {
        let seen = self.seen_ids.clone();
        let tx = event_tx.clone();
        self.handles
            .push(tokio::spawn(run_ssdp_discovery(seen, tx)));

        let seen = self.seen_ids.clone();
        let tx = event_tx.clone();
        self.handles
            .push(tokio::spawn(run_mdns_discovery(seen, tx)));
    }

    pub async fn stop(&mut self) {
        for handle in self.handles.drain(..) {
            handle.abort();
            let _ = handle.await;
        }
        self.seen_ids.lock().await.clear();
        tracing::info!("[discovery] Discovery stopped");
    }
}

async fn run_ssdp_discovery(
    seen_ids: Arc<Mutex<HashSet<String>>>,
    event_tx: mpsc::Sender<DiscoveryEvent>,
) {
    tracing::info!("[discovery] SSDP discovery started");
    let search_target: ssdp_client::SearchTarget =
        "urn:schemas-upnp-org:device:MediaRenderer:1".parse().unwrap();
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap_or_default();

    loop {
        match ssdp_client::search(&search_target, Duration::from_secs(5), 3, None).await {
            Ok(responses) => {
                use futures::StreamExt;
                tokio::pin!(responses);
                while let Some(Ok(response)) = responses.next().await {
                    let location = response.location().to_string();
                    // Extract host from the location URL
                    let host = reqwest::Url::parse(&location)
                        .ok()
                        .and_then(|u| u.host_str().map(|s| s.to_string()))
                        .unwrap_or_default();

                    handle_ssdp_device(
                        &client,
                        &location,
                        &host,
                        &seen_ids,
                        &event_tx,
                    )
                    .await;
                }
            }
            Err(e) => {
                tracing::warn!("[discovery] SSDP search failed: {}", e);
            }
        }

        tokio::time::sleep(Duration::from_secs(30)).await;
    }
}

async fn handle_ssdp_device(
    client: &reqwest::Client,
    location: &str,
    host: &str,
    seen_ids: &Arc<Mutex<HashSet<String>>>,
    event_tx: &mpsc::Sender<DiscoveryEvent>,
) {
    let resp = match client.get(location).send().await {
        Ok(r) => r,
        Err(_) => return,
    };
    let xml = match resp.text().await {
        Ok(t) => t,
        Err(_) => return,
    };

    if !upnp::is_media_renderer(&xml) {
        return;
    }

    let is_sonos = upnp::is_sonos_device(&xml);
    let is_teufel = upnp::is_teufel_device(&xml);
    let name = upnp::parse_friendly_name(&xml)
        .unwrap_or_else(|| format!("DLNA Device {}", host));

    let (device_type, id, display_name) = if is_sonos {
        (DeviceType::Sonos, format!("sonos-{}", host), name)
    } else {
        let prefix = if is_teufel {
            format!("Teufel {}", name)
        } else {
            name
        };
        (DeviceType::Teufel, format!("teufel-{}", host), prefix)
    };

    let port = reqwest::Url::parse(location)
        .ok()
        .and_then(|u| u.port())
        .unwrap_or(if is_sonos { 1400 } else { 80 });

    {
        let mut seen = seen_ids.lock().await;
        if seen.contains(&id) {
            return;
        }
        seen.insert(id.clone());
    }

    let config = DeviceConfig {
        id,
        name: display_name,
        host: host.to_string(),
        port,
        device_type,
        location: Some(location.to_string()),
        model: if is_teufel {
            Some("Teufel".to_string())
        } else if is_sonos {
            None
        } else {
            Some("DLNA".to_string())
        },
    };

    tracing::info!(
        "[discovery] Found {} device: {} ({}:{})",
        config.device_type,
        config.name,
        config.host,
        config.port
    );
    let _ = event_tx.send(DiscoveryEvent::DeviceFound(config)).await;
}

async fn run_mdns_discovery(
    seen_ids: Arc<Mutex<HashSet<String>>>,
    event_tx: mpsc::Sender<DiscoveryEvent>,
) {
    tracing::info!("[discovery] mDNS/AirPlay discovery started");

    let mdns = match mdns_sd::ServiceDaemon::new() {
        Ok(d) => d,
        Err(e) => {
            tracing::warn!("[discovery] mDNS daemon creation failed: {}", e);
            return;
        }
    };

    let receiver = match mdns.browse("_airplay._tcp.local.") {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!("[discovery] mDNS browse failed: {}", e);
            return;
        }
    };

    loop {
        match receiver.recv_async().await {
            Ok(mdns_sd::ServiceEvent::ServiceResolved(info)) => {
                let host = info
                    .get_addresses()
                    .iter()
                    .find(|a| a.is_ipv4())
                    .map(|a| a.to_string());

                let host = match host {
                    Some(h) => h,
                    None => continue,
                };

                let id = format!("airplay-{}", host);

                {
                    let mut seen = seen_ids.lock().await;
                    if seen.contains(&id) {
                        continue;
                    }
                    seen.insert(id.clone());
                }

                let name = info
                    .get_fullname()
                    .split('.')
                    .next()
                    .unwrap_or("AirPlay")
                    .to_string();

                let model = info
                    .get_properties()
                    .get_property_val_str("model")
                    .or_else(|| info.get_properties().get_property_val_str("am"))
                    .map(|s| s.to_string());

                let config = DeviceConfig {
                    id,
                    name: name.clone(),
                    host: host.clone(),
                    port: info.get_port(),
                    device_type: DeviceType::Airplay,
                    location: None,
                    model,
                };

                tracing::info!(
                    "[discovery] Found airplay device: {} ({}:{})",
                    name,
                    host,
                    info.get_port()
                );
                let _ = event_tx.send(DiscoveryEvent::DeviceFound(config)).await;
            }
            Ok(mdns_sd::ServiceEvent::ServiceRemoved(_, fullname)) => {
                // Try to find and remove the device
                let parts: Vec<&str> = fullname.split('.').collect();
                if let Some(name) = parts.first() {
                    // We can't easily map fullname back to host, so emit with name
                    tracing::info!("[discovery] Lost airplay device: {}", name);
                    // Device removal would need host mapping; for now log it
                }
            }
            Ok(_) => {} // Other events (searching, etc.)
            Err(e) => {
                tracing::warn!("[discovery] mDNS receive error: {}", e);
                break;
            }
        }
    }
}
