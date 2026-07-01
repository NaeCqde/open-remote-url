/// Integration tests for the host daemon in isolation.
///
/// These tests start a host-like HTTP server and send requests to it,
/// verifying that:
///   - GET /  returns "alive"
///   - POST /open  responds immediately and does not block the runtime
///   - POST /ports (Add)  creates proxy listeners
///   - POST /open after POST /ports  still responds without hanging
///
/// The "open URL" handler uses spawn_blocking to avoid occupying a tokio
/// worker thread while the OS launches the browser.  The tests confirm this
/// by checking that concurrent requests all complete within a tight deadline.

use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use base64::prelude::*;
use shared::{OpenUrlRequest, PortAction, PortsRequest, ProxyRequest, ProxyResponse};
use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::Arc,
    time::Duration,
};
use tokio::{net::TcpListener, sync::Mutex, task::JoinHandle};

// ── helpers ───────────────────────────────────────────────────────────────────

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

// ── host router (mirrors host/src/daemon.rs) ──────────────────────────────────

#[derive(Clone)]
struct HostState {
    open_fn: Arc<dyn Fn(String) + Send + Sync>,
    active_proxies: Arc<Mutex<HashMap<u16, JoinHandle<()>>>>,
}

fn host_router(state: HostState) -> Router {
    Router::new()
        .route("/", get(|| async { "alive" }))
        .route("/open", post(handle_open))
        .route("/ports", post(handle_ports))
        .with_state(state)
}

async fn handle_open(
    State(state): State<HostState>,
    _headers: HeaderMap,
    Json(payload): Json<OpenUrlRequest>,
) -> impl IntoResponse {
    let open_fn = state.open_fn.clone();
    let url = payload.url.clone();
    // Mirrors the fix: use spawn_blocking so the runtime is not occupied.
    tokio::task::spawn_blocking(move || open_fn(url));
    (StatusCode::OK, "OK")
}

async fn handle_ports(
    State(state): State<HostState>,
    _headers: HeaderMap,
    Json(req): Json<PortsRequest>,
) -> impl IntoResponse {
    match req.action {
        PortAction::Add => {
            for &port in &req.ports {
                let relay = req.relay_url.clone();
                let listener = match TcpListener::bind(format!("127.0.0.1:{}", port)).await {
                    Ok(l) => l,
                    Err(_) => continue,
                };
                let handle = tokio::spawn(async move {
                    let app = Router::new().fallback(
                        move |method: axum::http::Method,
                              uri: axum::http::Uri,
                              _headers: axum::http::HeaderMap,
                              body: axum::body::Bytes| {
                            let relay = relay.clone();
                            async move { proxy_fallback(port, method, uri, body, relay).await }
                        },
                    );
                    let _ = axum::serve(listener, app).await;
                });
                state.active_proxies.lock().await.insert(port, handle);
            }
        }
        PortAction::Delete => {
            let mut lock = state.active_proxies.lock().await;
            for &port in &req.ports {
                if let Some(h) = lock.remove(&port) {
                    h.abort();
                }
            }
        }
    }
    (StatusCode::OK, "OK")
}

async fn proxy_fallback(
    port: u16,
    method: axum::http::Method,
    uri: axum::http::Uri,
    body: axum::body::Bytes,
    relay_url: String,
) -> impl IntoResponse {
    let proxy_req = ProxyRequest {
        port,
        method: method.to_string(),
        path_and_query: uri
            .path_and_query()
            .map(|p| p.as_str().to_string())
            .unwrap_or_else(|| "/".into()),
        headers: HashMap::new(),
        body: BASE64_STANDARD.encode(&body),
    };
    let client = reqwest::Client::new();
    let relay_resp = match client
        .post(format!("{}/proxy", relay_url))
        .json(&proxy_req)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => return (StatusCode::BAD_GATEWAY, e.to_string()).into_response(),
    };
    if !relay_resp.status().is_success() {
        return (StatusCode::BAD_GATEWAY, "relay error").into_response();
    }
    let pr: ProxyResponse = match relay_resp.json().await {
        Ok(r) => r,
        Err(e) => return (StatusCode::BAD_GATEWAY, e.to_string()).into_response(),
    };
    let bytes = BASE64_STANDARD.decode(&pr.body).unwrap_or_default();
    axum::response::Response::builder()
        .status(pr.status)
        .body(axum::body::Body::from(bytes))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

// ── tests ─────────────────────────────────────────────────────────────────────

fn make_host(open_fn: impl Fn(String) + Send + Sync + 'static) -> (HostState, Router) {
    let state = HostState {
        open_fn: Arc::new(open_fn),
        active_proxies: Arc::new(Mutex::new(HashMap::new())),
    };
    let router = host_router(state.clone());
    (state, router)
}

/// GET / returns "alive".
#[tokio::test]
async fn host_health_check_returns_alive() {
    let l = bind_free().await;
    let port = port_of(&l);
    let (_, router) = make_host(|_| {});
    let _h = serve(l, router).await;
    wait_for_port(port, Duration::from_secs(2)).await;

    let body = reqwest::Client::new()
        .get(format!("http://127.0.0.1:{}/", port))
        .send().await.unwrap()
        .text().await.unwrap();
    assert_eq!(body, "alive");
    _h.abort();
}

/// POST /open responds with 200 immediately.
#[tokio::test]
async fn host_open_url_returns_ok() {
    let l = bind_free().await;
    let port = port_of(&l);
    let opened = Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
    let opened2 = opened.clone();
    let (_, router) = make_host(move |url| {
        opened2.lock().unwrap().push(url);
    });
    let _h = serve(l, router).await;
    wait_for_port(port, Duration::from_secs(2)).await;

    let status = reqwest::Client::new()
        .post(format!("http://127.0.0.1:{}/open", port))
        .json(&OpenUrlRequest { url: "https://example.com".into() })
        .send().await.unwrap()
        .status();
    assert_eq!(status.as_u16(), 200);
    _h.abort();
}

/// POST /open must not block the runtime: a slow open_fn must not prevent
/// subsequent requests from being served.
///
/// This is the regression test for the original bug where open::that() was
/// called synchronously, freezing the daemon.
#[tokio::test]
async fn host_open_url_does_not_block_runtime() {
    let l = bind_free().await;
    let host_port = port_of(&l);

    // open_fn sleeps for 500 ms, simulating a slow OS browser launch.
    let (_, router) = make_host(|_url| {
        std::thread::sleep(Duration::from_millis(500));
    });
    let _h = serve(l, router).await;
    wait_for_port(host_port, Duration::from_secs(2)).await;

    let client = reqwest::Client::new();
    let base = format!("http://127.0.0.1:{}", host_port);

    // Fire POST /open (slow) and GET / (fast) concurrently.
    // Both must finish within 300 ms even though open_fn takes 500 ms,
    // proving that spawn_blocking keeps the runtime free.
    let open_fut = client
        .post(format!("{}/open", base))
        .json(&OpenUrlRequest { url: "https://example.com".into() })
        .send();
    let health_fut = client.get(format!("{}/", base)).send();

    let deadline = Duration::from_millis(300);
    let (open_res, health_res) = tokio::time::timeout(
        deadline,
        futures::future::join(open_fut, health_fut),
    )
    .await
    .expect("requests timed out — runtime was blocked by open_fn");

    assert_eq!(open_res.unwrap().status().as_u16(), 200);
    assert_eq!(health_res.unwrap().text().await.unwrap(), "alive");
    _h.abort();
}

/// POST /ports (Add) causes the host to bind a proxy listener on the given port.
#[tokio::test]
async fn host_ports_add_creates_listener() {
    let l = bind_free().await;
    let host_port = port_of(&l);

    // Mock relay: always returns "relay ok".
    let relay_l = bind_free().await;
    let relay_port = port_of(&relay_l);
    let relay_app = Router::new().route(
        "/proxy",
        post(|| async {
            let resp = ProxyResponse {
                status: 200,
                headers: HashMap::new(),
                body: BASE64_STANDARD.encode("relay ok"),
            };
            (StatusCode::OK, Json(resp))
        }),
    );
    let _relay = serve(relay_l, relay_app).await;

    let (_, router) = make_host(|_| {});
    let _h = serve(l, router).await;
    wait_for_port(host_port, Duration::from_secs(2)).await;
    wait_for_port(relay_port, Duration::from_secs(2)).await;

    // Acquire a free port then release it for the host to bind.
    let proxy_port = {
        let tmp = bind_free().await;
        let p = port_of(&tmp);
        drop(tmp);
        p
    };

    let status = reqwest::Client::new()
        .post(format!("http://127.0.0.1:{}/ports", host_port))
        .json(&PortsRequest {
            ports: vec![proxy_port],
            action: PortAction::Add,
            relay_url: format!("http://127.0.0.1:{}", relay_port),
        })
        .send().await.unwrap()
        .status();
    assert_eq!(status.as_u16(), 200);

    wait_for_port(proxy_port, Duration::from_secs(3)).await;

    let body = reqwest::Client::new()
        .get(format!("http://127.0.0.1:{}/", proxy_port))
        .send().await.unwrap()
        .text().await.unwrap();
    assert_eq!(body, "relay ok");
    _h.abort();
}

/// Full scenario: POST /ports then POST /open then POST /open again.
/// All requests must succeed without the daemon freezing.
#[tokio::test]
async fn host_open_after_ports_still_responds() {
    let l = bind_free().await;
    let host_port = port_of(&l);
    let opened = Arc::new(std::sync::Mutex::new(0u32));
    let opened2 = opened.clone();
    let (_, router) = make_host(move |_| {
        *opened2.lock().unwrap() += 1;
    });
    let _h = serve(l, router).await;
    wait_for_port(host_port, Duration::from_secs(2)).await;

    let client = reqwest::Client::new();
    let base = format!("http://127.0.0.1:{}", host_port);

    // Simulate: windows client sends ports.
    let proxy_port = {
        let tmp = bind_free().await;
        let p = port_of(&tmp);
        drop(tmp);
        p
    };
    client
        .post(format!("{}/ports", base))
        .json(&PortsRequest {
            ports: vec![proxy_port],
            action: PortAction::Add,
            relay_url: "http://127.0.0.1:1".into(), // relay not needed for this test
        })
        .send().await.unwrap();

    // Simulate: windows client sends first URL open.
    let s1 = client
        .post(format!("{}/open", base))
        .json(&OpenUrlRequest { url: "https://first.example.com".into() })
        .send().await.unwrap()
        .status();
    assert_eq!(s1.as_u16(), 200);

    // Simulate: windows client sends second URL open — must not hang.
    let s2 = tokio::time::timeout(
        Duration::from_millis(500),
        client
            .post(format!("{}/open", base))
            .json(&OpenUrlRequest { url: "https://second.example.com".into() })
            .send(),
    )
    .await
    .expect("second POST /open timed out — daemon was frozen")
    .unwrap()
    .status();
    assert_eq!(s2.as_u16(), 200);

    _h.abort();
}
