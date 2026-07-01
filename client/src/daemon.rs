use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use base64::prelude::*;
use shared::{OpenUrlRequest, PortAction, PortsRequest, ProxyRequest, ProxyResponse};
use std::collections::{HashMap, HashSet, VecDeque};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

struct AppState {
    tx_open_url: mpsc::Sender<String>,
    passphrase: Option<String>,
}

#[derive(serde::Deserialize)]
struct OpenPayload {
    url: String,
}

pub async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let config = shared::config::ClientConfig::load();
    let client_host = config.client_host;
    let client_port = config.client_port;
    let passphrase = config.passphrase;
    let host_url = config.host_url;
    let relay_url = config.relay_url;

    let (tx_open_url, rx_open_url) = mpsc::channel::<String>(100);

    let state = Arc::new(AppState {
        tx_open_url,
        passphrase: passphrase.clone(),
    });

    // Start background port-tracking loop
    tokio::spawn(async move {
        if let Err(e) =
            run_port_tracker(rx_open_url, host_url, client_port, passphrase, relay_url).await
        {
            log::error!("Port tracker error: {}", e);
        }
    });

    let app = Router::new()
        .route("/", get(|| async { "alive" }))
        .route("/open", post(handle_open))
        .route("/proxy", post(handle_proxy))
        .with_state(state);

    let addr: SocketAddr = format!("{}:{}", client_host, client_port).parse()?;
    log::info!(
        "Client Daemon listening on http://{}:{}",
        client_host,
        client_port
    );
    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(l) => l,
        Err(e) => {
            if e.kind() == std::io::ErrorKind::AddrInUse {
                let msg = format!(
                    "Failed to start Client Daemon: Port {} is already in use. Another instance is likely running.",
                    client_port
                );
                log::error!("{}", msg);
                println!("=== Open Remote URL Daemon Error ===\n{}", msg);
                std::process::exit(1);
            } else {
                return Err(e.into());
            }
        }
    };
    axum::serve(listener, app).await?;

    Ok(())
}

async fn handle_open(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<OpenPayload>,
) -> impl IntoResponse {
    if let Some(ref phrase) = state.passphrase {
        if let Some(auth_val) = headers.get("Authorization") {
            if auth_val != &format!("Bearer {}", phrase) {
                return (StatusCode::UNAUTHORIZED, "Unauthorized").into_response();
            }
        } else {
            return (StatusCode::UNAUTHORIZED, "Unauthorized").into_response();
        }
    }

    log::info!("Received URL request: {}", payload.url);
    let _ = state.tx_open_url.send(payload.url).await;
    (StatusCode::OK, "OK").into_response()
}

async fn handle_proxy(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<ProxyRequest>,
) -> impl IntoResponse {
    if let Some(ref phrase) = state.passphrase {
        if let Some(auth_val) = headers.get("Authorization") {
            if auth_val != &format!("Bearer {}", phrase) {
                return (StatusCode::UNAUTHORIZED, "Unauthorized").into_response();
            }
        } else {
            return (StatusCode::UNAUTHORIZED, "Unauthorized").into_response();
        }
    }

    log::info!(
        "Proxying request for port {} - {} {}",
        req.port,
        req.method,
        req.path_and_query
    );

    let local_url = format!("http://localhost:{}{}", req.port, req.path_and_query);

    let body_bytes = match BASE64_STANDARD.decode(&req.body) {
        Ok(b) => b,
        Err(e) => {
            log::error!("Failed to decode base64 body: {}", e);
            return (StatusCode::BAD_REQUEST, "Invalid base64 body").into_response();
        }
    };

    // Do NOT follow redirects: return them as-is so the browser can handle them.
    // If the relay followed a 302 to a URL that is unreachable (e.g. the local
    // OAuth server shuts down after processing the callback), reqwest would fail
    // internally and the browser would never see the redirect Location.
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(std::time::Duration::from_secs(10))
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());
    let method = match reqwest::Method::from_bytes(req.method.as_bytes()) {
        Ok(m) => m,
        Err(_) => return (StatusCode::BAD_REQUEST, "Invalid HTTP method").into_response(),
    };

    let mut req_builder = client.request(method, &local_url).body(body_bytes);

    for (key, val) in &req.headers {
        if key.to_lowercase() != "host" {
            req_builder = req_builder.header(key, val);
        }
    }

    match req_builder.send().await {
        Ok(resp) => {
            let status = resp.status().as_u16();
            let mut headers_map = HashMap::new();
            for (key, val) in resp.headers().iter() {
                if let Ok(val_str) = val.to_str() {
                    headers_map.insert(key.to_string(), val_str.to_string());
                }
            }

            let body_bytes = resp.bytes().await.unwrap_or_default();
            let body_base64 = BASE64_STANDARD.encode(&body_bytes);

            let proxy_resp = ProxyResponse {
                status,
                headers: headers_map,
                body: body_base64,
            };

            (StatusCode::OK, Json(proxy_resp)).into_response()
        }
        Err(e) => {
            log::error!(
                "Failed to forward request to local port {}: {}",
                req.port,
                e
            );
            (
                StatusCode::BAD_GATEWAY,
                format!("Local connection error: {}", e),
            )
                .into_response()
        }
    }
}

async fn run_port_tracker(
    mut rx: mpsc::Receiver<String>,
    host_url: String,
    client_port: u16,
    passphrase: Option<String>,
    relay_url: String,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut history: VecDeque<(Instant, HashSet<u16>)> = VecDeque::new();
    let mut interval = tokio::time::interval(Duration::from_millis(500));
    let sent_ports = Arc::new(std::sync::Mutex::new(HashSet::<u16>::new()));

    let resolved_relay_url = relay_url;

    loop {
        tokio::select! {
            _ = interval.tick() => {
                if let Ok(ports) = get_listening_ports() {
                    history.push_back((Instant::now(), ports.clone()));

                    // Check if any sent ports are closed
                    let mut closed_ports = Vec::new();
                    {
                        let mut sent_lock = sent_ports.lock().unwrap();
                        for &port in sent_lock.iter() {
                            if !ports.contains(&port) {
                                closed_ports.push(port);
                            }
                        }
                        for port in &closed_ports {
                            sent_lock.remove(port);
                        }
                    }

                    if !closed_ports.is_empty() {
                        log::info!("Detected closed ports: {:?}", closed_ports);
                        let host_url_clone = host_url.clone();
                        let passphrase_clone = passphrase.clone();
                        let resolved_relay_url_clone = resolved_relay_url.clone();
                        tokio::spawn(async move {
                            if let Err(e) = send_ports_update(&host_url_clone, closed_ports, PortAction::Delete, &resolved_relay_url_clone, &passphrase_clone).await {
                                log::error!("Failed to send delete ports request to host: {}", e);
                            }
                        });
                    }
                }
                while history.front().map_or(false, |(t, _)| t.elapsed() > Duration::from_secs(15)) {
                    history.pop_front();
                }
            }
            Some(url) = rx.recv() => {
                let host_url_clone = host_url.clone();
                let passphrase_clone = passphrase.clone();
                let sent_ports_clone = sent_ports.clone();
                let resolved_relay_url_clone = resolved_relay_url.clone();

                // Get all ports seen in the last 15 seconds (history + current)
                let mut ports_15s = HashSet::new();
                for (_, ports) in &history {
                    for &port in ports {
                        ports_15s.insert(port);
                    }
                }
                if let Ok(current_ports) = get_listening_ports() {
                    for port in current_ports {
                        ports_15s.insert(port);
                    }
                }
                ports_15s.remove(&client_port);

                tokio::spawn(async move {
                    // 1. Immediately send open request to Host Daemon (with empty ports)
                    log::info!("Sending immediate open request for URL: {}", url);
                    let open_payload = OpenUrlRequest { url };

                    if let Err(e) = send_open_request(&host_url_clone, &open_payload, &passphrase_clone).await {
                        log::error!("Failed to send open request: {}", e);
                    }

                    // 2. Send 15-second ports to host /ports (action: add)
                    if !ports_15s.is_empty() {
                        let ports_list: Vec<u16> = ports_15s.into_iter().collect();
                        log::info!("Sending ports allocated up to 15s ago: {:?}", ports_list);
                        match send_ports_update(&host_url_clone, ports_list.clone(), PortAction::Add, &resolved_relay_url_clone, &passphrase_clone).await {
                            Ok(_) => {
                                let mut sent_lock = sent_ports_clone.lock().unwrap();
                                for port in ports_list {
                                    sent_lock.insert(port);
                                }
                            }
                            Err(e) => {
                                log::error!("Failed to send 15s ports update to host: {}", e);
                            }
                        }
                    }

                    // 3. Wait 1 second, check for new ports, and send. Repeat 3 times.
                    for i in 1..=3 {
                        tokio::time::sleep(Duration::from_secs(1)).await;
                        let current_ports = get_listening_ports().unwrap_or_default();
                        let mut new_ports = Vec::new();
                        {
                            let sent_lock = sent_ports_clone.lock().unwrap();
                            for port in current_ports {
                                if port != client_port && !sent_lock.contains(&port) {
                                    new_ports.push(port);
                                }
                            }
                        }

                        if !new_ports.is_empty() {
                            log::info!("Check #{}: Detected newly opened ports: {:?}", i, new_ports);
                            match send_ports_update(&host_url_clone, new_ports.clone(), PortAction::Add, &resolved_relay_url_clone, &passphrase_clone).await {
                                Ok(_) => {
                                    let mut sent_lock = sent_ports_clone.lock().unwrap();
                                    for port in new_ports {
                                        sent_lock.insert(port);
                                    }
                                }
                                Err(e) => {
                                    log::error!("Failed to send new ports update to host (check #{}): {}", i, e);
                                }
                            }
                        }
                    }
                });
            }
        }
    }
}

async fn send_open_request(
    host_url: &str,
    payload: &OpenUrlRequest,
    passphrase: &Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();
    let mut req = client.post(format!("{}/open", host_url)).json(payload);
    if let Some(ref phrase) = passphrase {
        req = req.header("Authorization", format!("Bearer {}", phrase));
    }
    let resp = req.send().await?;
    if !resp.status().is_success() {
        return Err(format!("HTTP error: {}", resp.status()).into());
    }
    Ok(())
}

async fn send_ports_update(
    host_url: &str,
    ports: Vec<u16>,
    action: PortAction,
    relay_url: &str,
    passphrase: &Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();
    let payload = PortsRequest {
        ports,
        action,
        relay_url: relay_url.to_string(),
    };
    let mut req = client.post(format!("{}/ports", host_url)).json(&payload);
    if let Some(ref phrase) = passphrase {
        req = req.header("Authorization", format!("Bearer {}", phrase));
    }
    let resp = req.send().await?;
    if !resp.status().is_success() {
        return Err(format!("HTTP error: {}", resp.status()).into());
    }
    Ok(())
}

fn get_listening_ports() -> Result<HashSet<u16>, Box<dyn std::error::Error>> {
    let mut ports = HashSet::new();
    if let Ok(listeners) = listeners::get_all() {
        for listener in listeners {
            ports.insert(listener.socket.port());
        }
    }
    Ok(ports)
}
