use axum::{
    body::Bytes,
    extract::State,
    http::{HeaderMap, Method, StatusCode, Uri},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use base64::prelude::*;
use shared::{OpenUrlRequest, PortAction, PortsRequest, ProxyRequest, ProxyResponse};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

struct ProxyInstance {
    shutdown_tx: tokio::sync::oneshot::Sender<()>,
    id: u64,
}

struct ActiveProxies {
    map: HashMap<u16, ProxyInstance>,
}

struct ServerState {
    passphrase: Option<String>,
    active_proxies: Arc<Mutex<ActiveProxies>>,
    next_proxy_id: std::sync::atomic::AtomicU64,
}

#[derive(Clone)]
struct ProxyState {
    port: u16,
    relay_url: String,
    passphrase: Option<String>,
    // Shared across all requests for this proxy.  Built once with timeouts so
    // that a slow or unreachable relay does not hold file-descriptors forever.
    client: reqwest::Client,
}

pub async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let config = shared::config::HostConfig::load();
    let bind_host = config.bind_host;
    let port = config.port;
    let passphrase = config.passphrase;

    let state = Arc::new(ServerState {
        passphrase,
        active_proxies: Arc::new(Mutex::new(ActiveProxies {
            map: HashMap::new(),
        })),
        next_proxy_id: std::sync::atomic::AtomicU64::new(0),
    });

    let app = Router::new()
        .route("/", get(|| async { "alive" }))
        .route("/open", post(handle_open_url))
        .route("/ports", post(handle_ports))
        .with_state(state);

    let addr: SocketAddr = format!("{}:{}", bind_host, port).parse()?;
    log::info!("Host Daemon listening on http://{}:{}", bind_host, port);
    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(l) => l,
        Err(e) => {
            if e.kind() == std::io::ErrorKind::AddrInUse {
                let msg = format!(
                    "Failed to start Host Daemon: Port {} is already in use. Another instance is likely running.",
                    port
                );
                log::error!("{}", msg);
                println!("=== Open Remote URL Host Daemon Error ===\n{}", msg);
                std::process::exit(1);
            } else {
                return Err(e.into());
            }
        }
    };
    axum::serve(listener, app).await?;

    Ok(())
}

async fn handle_open_url(
    State(state): State<Arc<ServerState>>,
    headers: HeaderMap,
    Json(payload): Json<OpenUrlRequest>,
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

    log::info!("Received request to open URL: {}", payload.url);

    // open::that() is synchronous and can block while the OS launches the
    // browser.  Run it on a dedicated thread so the tokio runtime stays free
    // to process other requests (ports updates, subsequent opens, etc.).
    let url = payload.url.clone();
    tokio::task::spawn_blocking(move || match open::that(&url) {
        Ok(_) => log::info!("Successfully opened URL on Host browser"),
        Err(e) => log::error!("Failed to open URL on Host browser: {}", e),
    });

    (StatusCode::OK, "OK").into_response()
}

async fn handle_ports(
    State(state): State<Arc<ServerState>>,
    headers: HeaderMap,
    Json(payload): Json<PortsRequest>,
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
        "Received ports request: action={:?}, ports={:?}, relay_url={}",
        payload.action,
        payload.ports,
        payload.relay_url
    );

    match payload.action {
        PortAction::Add => {
            for &port in &payload.ports {
                start_proxy(port, payload.relay_url.clone(), state.clone()).await;
            }
        }
        PortAction::Delete => {
            let mut lock = state.active_proxies.lock().await;
            for &port in &payload.ports {
                if let Some(instance) = lock.map.remove(&port) {
                    log::info!("Removing proxy for port {} due to delete request.", port);
                    let _ = instance.shutdown_tx.send(());
                }
            }
        }
    }

    (StatusCode::OK, "OK").into_response()
}

async fn start_proxy(port: u16, relay_url: String, state: Arc<ServerState>) {
    let mut lock = state.active_proxies.lock().await;
    if lock.map.contains_key(&port) {
        log::info!("Proxy for port {} already active. Reusing existing.", port);
        return;
    }

    let proxy_id = state
        .next_proxy_id
        .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
    lock.map.insert(
        port,
        ProxyInstance {
            shutdown_tx,
            id: proxy_id,
        },
    );

    let active_proxies_clone = state.active_proxies.clone();
    let passphrase_clone = state.passphrase.clone();
    let relay_url_clone = relay_url.clone();
    let relay_client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(30))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());

    tokio::spawn(async move {
        log::info!("Starting temporary proxy server on 127.0.0.1:{}", port);
        let app = Router::new()
            .fallback(handle_proxy_fallback)
            .with_state(ProxyState {
                port,
                relay_url: relay_url_clone,
                passphrase: passphrase_clone,
                client: relay_client,
            });

        // Use 127.0.0.1 to avoid runtime panic with 127.0.0.1 parse
        let addr: SocketAddr = format!("127.0.0.1:{}", port).parse().unwrap();
        let listener = match tokio::net::TcpListener::bind(addr).await {
            Ok(l) => l,
            Err(e) => {
                log::error!("Failed to bind proxy to port {}: {}", port, e);
                let mut lk = active_proxies_clone.lock().await;
                if let Some(instance) = lk.map.get(&port) {
                    if instance.id == proxy_id {
                        lk.map.remove(&port);
                    }
                }
                return;
            }
        };

        let server = axum::serve(listener, app).with_graceful_shutdown(async move {
            let _ = shutdown_rx.await;
            log::info!("Shutting down proxy server on port {}", port);
        });

        if let Err(e) = server.await {
            log::error!("Proxy server error on port {}: {}", port, e);
        }
    });

    // Spawn a timeout task to close this proxy after 5 minutes
    let active_proxies_clone = state.active_proxies.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(300)).await;
        let mut lock = active_proxies_clone.lock().await;
        if let Some(instance) = lock.map.get(&port) {
            if instance.id == proxy_id {
                if let Some(removed_instance) = lock.map.remove(&port) {
                    log::info!(
                        "Timeout reached (5 minutes) for port {}. Shutting down proxy.",
                        port
                    );
                    let _ = removed_instance.shutdown_tx.send(());
                }
            }
        }
    });
}

async fn handle_proxy_fallback(
    State(state): State<ProxyState>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    log::info!(
        "[Proxy:{}] Received request on Host: {} {}",
        state.port,
        method,
        uri
    );

    let mut headers_map = HashMap::new();
    for (key, val) in headers.iter() {
        if let Ok(val_str) = val.to_str() {
            headers_map.insert(key.to_string(), val_str.to_string());
        }
    }

    let body_base64 = BASE64_STANDARD.encode(&body);
    let path_and_query = uri
        .path_and_query()
        .map(|pq| pq.as_str().to_string())
        .unwrap_or_else(|| "/".to_string());

    let proxy_req = ProxyRequest {
        port: state.port,
        method: method.to_string(),
        path_and_query,
        headers: headers_map,
        body: body_base64,
    };

    let mut post_req = state.client
        .post(format!("{}/proxy", state.relay_url))
        .json(&proxy_req);
    if let Some(ref phrase) = state.passphrase {
        post_req = post_req.header("Authorization", format!("Bearer {}", phrase));
    }

    match post_req.send().await {
        Ok(resp) => {
            let status = resp.status();
            if status.is_success() {
                if let Ok(proxy_resp) = resp.json::<ProxyResponse>().await {
                    let mut response_builder =
                        axum::response::Response::builder().status(proxy_resp.status);

                    for (k, v) in proxy_resp.headers {
                        if k.to_lowercase() != "content-length"
                            && k.to_lowercase() != "transfer-encoding"
                        {
                            if let Ok(hname) = axum::http::HeaderName::from_bytes(k.as_bytes()) {
                                if let Ok(hval) = axum::http::HeaderValue::from_str(&v) {
                                    response_builder = response_builder.header(hname, hval);
                                }
                            }
                        }
                    }

                    if let Ok(body_bytes) = BASE64_STANDARD.decode(&proxy_resp.body) {
                        return response_builder
                            .body(axum::body::Body::from(body_bytes))
                            .unwrap_or_else(|_| {
                                (
                                    StatusCode::INTERNAL_SERVER_ERROR,
                                    "Failed to build response body",
                                )
                                    .into_response()
                            });
                    }
                }
            }
            log::error!(
                "[Proxy:{}] Client relay returned error: {}",
                state.port,
                status
            );
            (StatusCode::BAD_GATEWAY, "Client relay returned error").into_response()
        }
        Err(e) => {
            log::error!(
                "[Proxy:{}] Failed to connect to Client relay: {}",
                state.port,
                e
            );
            (StatusCode::BAD_GATEWAY, "Failed to connect to Client relay").into_response()
        }
    }
}
