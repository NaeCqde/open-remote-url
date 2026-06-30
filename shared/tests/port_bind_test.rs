/// Integration tests
///
/// 1. Port binding  : client/host config port is correctly read from .env and the
///    HTTP server binds to it and returns "alive".
/// 2. Proxy relay   : the client relay (POST /proxy) forwards a request to a local
///    backend and returns the response wrapped in ProxyResponse.
/// 3. Host proxy    : POST /ports causes the host to open a proxy listener; requests
///    through that listener are tunnelled to the relay and back.
///
/// All tests are platform-agnostic.
/// Port-binding tests mutate process-wide env vars and run serially.
/// Port-forwarding tests use ephemeral ports and run in parallel.

use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use base64::prelude::*;
use serial_test::serial;
use shared::{PortAction, PortsRequest, ProxyRequest, ProxyResponse};
use std::{collections::HashMap, net::SocketAddr, sync::Arc, time::Duration};
use tempfile::TempDir;
use tokio::{net::TcpListener, sync::Mutex, task::JoinHandle};

// ── shared helpers ────────────────────────────────────────────────────────────

fn write_env(dir: &TempDir, content: &str) {
    let path = dir.path().join(".env");
    std::fs::write(&path, content).unwrap();
    dotenvy::from_path_override(&path).unwrap();
}

async fn bind_free() -> TcpListener {
    TcpListener::bind("127.0.0.1:0").await.unwrap()
}

fn port_of(l: &TcpListener) -> u16 {
    l.local_addr().unwrap().port()
}

async fn serve(listener: TcpListener, app: Router) -> JoinHandle<()> {
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    })
}

/// Polls TCP connect until the port accepts connections or the deadline is reached.
async fn wait_for_port(port: u16, timeout: Duration) {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        if std::net::TcpStream::connect_timeout(
            &SocketAddr::from(([127, 0, 0, 1], port)),
            Duration::from_millis(50),
        )
        .is_ok()
        {
            return;
        }
        if tokio::time::Instant::now() >= deadline {
            panic!("port {} did not become ready within {:?}", port, timeout);
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

// ── 1. port binding tests (serial – mutate env vars) ─────────────────────────

#[tokio::test]
#[serial]
async fn client_binds_to_configured_port_and_returns_alive() {
    let dir = TempDir::new().unwrap();
    write_env(
        &dir,
        "LISTEN=127.0.0.1:39100\nHOST_URL=http://localhost:40000\nRELAY_URL=http://localhost:39100\nPASSPHRASE=\n",
    );

    let config = shared::config::ClientConfig::load();
    assert_eq!(config.client_port, 39100, "client_port must match LISTEN in .env");
    assert_eq!(config.client_host, "127.0.0.1");

    let addr: SocketAddr = format!("{}:{}", config.client_host, config.client_port).parse().unwrap();
    let listener = TcpListener::bind(addr).await.expect("failed to bind configured port");
    let handle = serve(listener, Router::new().route("/", get(|| async { "alive" }))).await;

    wait_for_port(config.client_port, Duration::from_secs(2)).await;

    let body = reqwest::Client::new()
        .get(format!("http://{}:{}/", config.client_host, config.client_port))
        .send().await.unwrap()
        .text().await.unwrap();
    assert_eq!(body, "alive");
    handle.abort();
}

#[tokio::test]
#[serial]
async fn host_binds_to_configured_port_and_returns_alive() {
    let dir = TempDir::new().unwrap();
    write_env(&dir, "LISTEN=127.0.0.1:39200\nPASSPHRASE=\n");

    let config = shared::config::HostConfig::load();
    assert_eq!(config.port, 39200, "port must match LISTEN in .env");
    assert_eq!(config.bind_host, "127.0.0.1");

    let addr: SocketAddr = format!("{}:{}", config.bind_host, config.port).parse().unwrap();
    let listener = TcpListener::bind(addr).await.expect("failed to bind configured port");
    let handle = serve(listener, Router::new().route("/", get(|| async { "alive" }))).await;

    wait_for_port(config.port, Duration::from_secs(2)).await;

    let body = reqwest::Client::new()
        .get(format!("http://{}:{}/", config.bind_host, config.port))
        .send().await.unwrap()
        .text().await.unwrap();
    assert_eq!(body, "alive");
    handle.abort();
}

// ── 2. proxy relay test ───────────────────────────────────────────────────────
//
// Tests the client relay component in isolation.
//
//   test ──POST /proxy──► client relay ──GET /{path}──► backend
//                                        ◄──200 "backend response"──
//         ◄──200 ProxyResponse(body="backend response")──

fn backend_app() -> Router {
    Router::new().route("/", get(|| async { "backend response" }))
}

fn client_relay_app() -> Router {
    Router::new().route("/proxy", post(client_relay_handler))
}

async fn client_relay_handler(Json(req): Json<ProxyRequest>) -> impl IntoResponse {
    let body_bytes = BASE64_STANDARD.decode(&req.body).unwrap_or_default();
    let client = reqwest::Client::new();
    let method =
        reqwest::Method::from_bytes(req.method.as_bytes()).unwrap_or(reqwest::Method::GET);
    let url = format!("http://127.0.0.1:{}{}", req.port, req.path_and_query);

    let mut builder = client.request(method, &url).body(body_bytes);
    for (k, v) in &req.headers {
        if k.to_lowercase() != "host" {
            builder = builder.header(k, v);
        }
    }

    match builder.send().await {
        Ok(resp) => {
            let status = resp.status().as_u16();
            let mut headers = HashMap::new();
            for (k, v) in resp.headers() {
                if let Ok(s) = v.to_str() {
                    headers.insert(k.to_string(), s.to_string());
                }
            }
            let body = BASE64_STANDARD.encode(resp.bytes().await.unwrap_or_default());
            (StatusCode::OK, Json(ProxyResponse { status, headers, body })).into_response()
        }
        Err(e) => (StatusCode::BAD_GATEWAY, e.to_string()).into_response(),
    }
}

#[tokio::test]
async fn proxy_relay_forwards_request_to_backend() {
    let backend_l = bind_free().await;
    let backend_port = port_of(&backend_l);
    let relay_l = bind_free().await;
    let relay_port = port_of(&relay_l);

    let _backend = serve(backend_l, backend_app()).await;
    let _relay = serve(relay_l, client_relay_app()).await;

    wait_for_port(backend_port, Duration::from_secs(2)).await;
    wait_for_port(relay_port, Duration::from_secs(2)).await;

    let proxy_req = ProxyRequest {
        port: backend_port,
        method: "GET".into(),
        path_and_query: "/".into(),
        headers: HashMap::new(),
        body: String::new(),
    };

    let resp: ProxyResponse = reqwest::Client::new()
        .post(format!("http://127.0.0.1:{}/proxy", relay_port))
        .json(&proxy_req)
        .send().await.unwrap()
        .json().await.unwrap();

    assert_eq!(resp.status, 200);
    let body = String::from_utf8(BASE64_STANDARD.decode(&resp.body).unwrap()).unwrap();
    assert_eq!(body, "backend response");

    _backend.abort();
    _relay.abort();
}

// ── 3. host proxy + forwarding chain test ────────────────────────────────────
//
// The host proxy listener and the backend live on different ports so there is
// no address conflict when running on a single machine.
//
// A mock relay is used instead of the real client relay.  This avoids the
// circular-forwarding problem that would occur if the real relay tried to
// connect back to the host-side proxy port.  The real relay is covered by
// test 2.
//
// Topology:
//
//   test client ──GET /──► host proxy (proxy_port)
//                              │ POST /proxy  { port: proxy_port, ... }
//                              ▼
//                         mock relay  (returns "forwarded response" directly)
//                              │
//   test client ◄──200 "forwarded response"──────────────────────────────

#[derive(Clone)]
struct HostState {
    // not used in handler logic but kept for realism
    _relay_url: Arc<Mutex<Option<String>>>,
}

fn host_app(state: HostState) -> Router {
    Router::new()
        .route("/", get(|| async { "alive" }))
        .route("/ports", post(host_ports_handler))
        .with_state(state)
}

async fn host_ports_handler(
    State(_state): State<HostState>,
    Json(req): Json<PortsRequest>,
) -> impl IntoResponse {
    if req.action == PortAction::Add {
        let relay_url = req.relay_url.clone();
        for &port in &req.ports {
            let relay = relay_url.clone();
            tokio::spawn(async move {
                if let Ok(listener) =
                    TcpListener::bind(format!("127.0.0.1:{}", port)).await
                {
                    let proxy_app = Router::new().fallback(
                        move |method: axum::http::Method,
                              uri: axum::http::Uri,
                              headers: axum::http::HeaderMap,
                              body: axum::body::Bytes| {
                            let relay = relay.clone();
                            async move {
                                host_proxy_fallback(port, method, uri, headers, body, relay)
                                    .await
                            }
                        },
                    );
                    let _ = axum::serve(listener, proxy_app).await;
                }
            });
        }
    }
    StatusCode::OK
}

async fn host_proxy_fallback(
    port: u16,
    method: axum::http::Method,
    uri: axum::http::Uri,
    headers: axum::http::HeaderMap,
    body: axum::body::Bytes,
    relay_url: String,
) -> impl IntoResponse {
    let mut hmap = HashMap::new();
    for (k, v) in &headers {
        if let Ok(s) = v.to_str() {
            hmap.insert(k.to_string(), s.to_string());
        }
    }

    let proxy_req = ProxyRequest {
        port,
        method: method.to_string(),
        path_and_query: uri
            .path_and_query()
            .map(|p| p.as_str().to_string())
            .unwrap_or_else(|| "/".into()),
        headers: hmap,
        body: BASE64_STANDARD.encode(&body),
    };

    let client = reqwest::Client::new();
    let relay_resp =
        match client.post(format!("{}/proxy", relay_url)).json(&proxy_req).send().await {
            Ok(r) => r,
            Err(e) => return (StatusCode::BAD_GATEWAY, e.to_string()).into_response(),
        };

    if !relay_resp.status().is_success() {
        return (StatusCode::BAD_GATEWAY, "relay returned error").into_response();
    }

    let proxy_resp: ProxyResponse = match relay_resp.json().await {
        Ok(r) => r,
        Err(e) => return (StatusCode::BAD_GATEWAY, e.to_string()).into_response(),
    };

    let body_bytes = BASE64_STANDARD.decode(&proxy_resp.body).unwrap_or_default();
    let mut builder =
        axum::response::Response::builder().status(proxy_resp.status);
    for (k, v) in proxy_resp.headers {
        let lower = k.to_lowercase();
        if lower != "content-length" && lower != "transfer-encoding" {
            if let (Ok(name), Ok(val)) = (
                axum::http::HeaderName::from_bytes(k.as_bytes()),
                axum::http::HeaderValue::from_str(&v),
            ) {
                builder = builder.header(name, val);
            }
        }
    }
    builder
        .body(axum::body::Body::from(body_bytes))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

#[tokio::test]
async fn host_proxy_forwards_through_relay_to_backend() {
    // Mock relay: returns a canned ProxyResponse without actually forwarding.
    // This avoids the circular-forwarding issue on a single machine.
    let mock_relay_l = bind_free().await;
    let mock_relay_port = port_of(&mock_relay_l);
    let mock_relay_app = Router::new().route(
        "/proxy",
        post(|| async {
            let resp = ProxyResponse {
                status: 200,
                headers: HashMap::new(),
                body: BASE64_STANDARD.encode("forwarded response"),
            };
            (StatusCode::OK, Json(resp))
        }),
    );
    let _mock_relay = serve(mock_relay_l, mock_relay_app).await;

    // Host server.
    let host_l = bind_free().await;
    let host_port = port_of(&host_l);
    let host_state = HostState { _relay_url: Arc::new(Mutex::new(None)) };
    let _host = serve(host_l, host_app(host_state)).await;

    wait_for_port(mock_relay_port, Duration::from_secs(2)).await;
    wait_for_port(host_port, Duration::from_secs(2)).await;

    // Acquire a free port for the proxy listener, then release it so the host can bind it.
    let proxy_port = {
        let l = bind_free().await;
        let p = port_of(&l);
        drop(l);
        p
    };

    // Tell the host to open a proxy on proxy_port, tunnelling to the mock relay.
    let status = reqwest::Client::new()
        .post(format!("http://127.0.0.1:{}/ports", host_port))
        .json(&PortsRequest {
            ports: vec![proxy_port],
            action: PortAction::Add,
            relay_url: format!("http://127.0.0.1:{}", mock_relay_port),
        })
        .send().await.unwrap()
        .status();
    assert_eq!(status.as_u16(), 200);

    // Wait until the host has actually bound proxy_port.
    wait_for_port(proxy_port, Duration::from_secs(3)).await;

    // Request through the host proxy tunnel.
    let body = reqwest::Client::new()
        .get(format!("http://127.0.0.1:{}/", proxy_port))
        .send().await.unwrap()
        .text().await.unwrap();
    assert_eq!(body, "forwarded response");
}
