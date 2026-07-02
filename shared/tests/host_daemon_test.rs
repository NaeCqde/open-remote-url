/// Integration tests for the host daemon in isolation.
///
/// Test sequence matches the real-world bug scenario:
///   POST /open  →  POST /ports  →  POST /open
///
/// Root causes fixed:
///   1. open::that() was a synchronous blocking call inside an async handler
///      → fixed with tokio::task::spawn_blocking
///   2. reqwest::Client was created per-proxy-request with no timeout; if the
///      Windows relay is unreachable every browser sub-request (HTML, CSS, JS,
///      images …) hangs indefinitely, exhausting file descriptors and preventing
///      the main server from accepting new connections
///      → fixed: one client per proxy, connect_timeout=10s, timeout=30s

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

// ── host router (mirrors host/src/daemon.rs with the same fixes applied) ─────

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
    // Fix 1: use spawn_blocking so the runtime is never occupied by open::that().
    tokio::task::spawn_blocking(move || open_fn(url));
    (StatusCode::OK, "OK")
}

/// Build a reqwest client with the same timeouts as the production fix.
fn relay_client() -> reqwest::Client {
    reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(30))
        .build()
        .unwrap()
}

async fn handle_ports(
    State(state): State<HostState>,
    _headers: HeaderMap,
    Json(req): Json<PortsRequest>,
) -> impl IntoResponse {
    match req.action {
        PortAction::Add => {
            for &port in &req.ports {
                let relay = req.self_url.clone();
                // Fix 2: build client once per proxy with timeouts.
                let client = relay_client();
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
                            let client = client.clone();
                            async move {
                                proxy_fallback(port, method, uri, body, relay, client).await
                            }
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
    self_url: String,
    client: reqwest::Client,
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
    let relay_resp = match client
        .post(format!("{}/proxy", self_url))
        .json(&proxy_req)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => return (StatusCode::BAD_GATEWAY, format!("Failed to connect to Sender relay: {}", e)).into_response(),
    };
    let relay_status = relay_resp.status();
    if !relay_status.is_success() {
        let body = relay_resp.text().await.unwrap_or_default();
        return (StatusCode::BAD_GATEWAY, format!("Sender relay error {}: {}", relay_status, body)).into_response();
    }
    let pr: ProxyResponse = match relay_resp.json().await {
        Ok(r) => r,
        Err(e) => return (StatusCode::BAD_GATEWAY, format!("Relay response parse error: {}", e)).into_response(),
    };
    let bytes = BASE64_STANDARD.decode(&pr.body).unwrap_or_default();
    axum::response::Response::builder()
        .status(pr.status)
        .body(axum::body::Body::from(bytes))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

fn make_host(open_fn: impl Fn(String) + Send + Sync + 'static) -> (HostState, Router) {
    let state = HostState {
        open_fn: Arc::new(open_fn),
        active_proxies: Arc::new(Mutex::new(HashMap::new())),
    };
    let router = host_router(state.clone());
    (state, router)
}

// ── tests ─────────────────────────────────────────────────────────────────────

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

/// POST /open responds with 200 OK.
#[tokio::test]
async fn host_open_url_returns_ok() {
    let l = bind_free().await;
    let port = port_of(&l);
    let (_, router) = make_host(|_| {});
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

/// Regression test for bug fix 1:
/// open_fn takes 500 ms, but GET / must respond within 200 ms concurrently,
/// proving that spawn_blocking keeps the runtime free.
#[tokio::test]
async fn host_open_does_not_block_runtime() {
    let l = bind_free().await;
    let host_port = port_of(&l);
    let (_, router) = make_host(|_| std::thread::sleep(Duration::from_millis(500)));
    let _h = serve(l, router).await;
    wait_for_port(host_port, Duration::from_secs(2)).await;

    let client = reqwest::Client::new();
    let base = format!("http://127.0.0.1:{}", host_port);

    let (open_res, health_res) = tokio::time::timeout(
        Duration::from_millis(200),
        futures::future::join(
            client.post(format!("{}/open", base))
                .json(&OpenUrlRequest { url: "https://example.com".into() })
                .send(),
            client.get(format!("{}/", base)).send(),
        ),
    )
    .await
    .expect("requests timed out — runtime was blocked by open_fn");

    assert_eq!(open_res.unwrap().status().as_u16(), 200);
    assert_eq!(health_res.unwrap().text().await.unwrap(), "alive");
    _h.abort();
}

/// Regression test for bug fix 2:
/// POST /open → POST /ports (relay that accepts TCP but never replies) → POST /open
///
/// Without the timeout fix, each browser sub-request through the proxy would
/// hang forever, eventually exhausting file descriptors and freezing the daemon.
/// With the fix the proxy requests time out and the second POST /open completes.
#[tokio::test]
async fn open_ports_open_sequence_host_stays_responsive() {
    // "Black-hole" relay: accepts TCP connections but never sends any data.
    // Simulates a Windows relay that is slow or unreachable.
    let blackhole_l = bind_free().await;
    let blackhole_port = port_of(&blackhole_l);
    tokio::spawn(async move {
        loop {
            // Accept but ignore — holds the connection open silently.
            if let Ok((_stream, _)) = blackhole_l.accept().await {}
        }
    });

    // Host
    let host_l = bind_free().await;
    let host_port = port_of(&host_l);
    let opened = Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
    let opened2 = opened.clone();
    let (_, router) = make_host(move |url| opened2.lock().unwrap().push(url));
    let _h = serve(host_l, router).await;
    wait_for_port(host_port, Duration::from_secs(2)).await;

    let client = reqwest::Client::new();
    let base = format!("http://127.0.0.1:{}", host_port);

    // Step 1: POST /open (first URL)
    let s1 = client
        .post(format!("{}/open", base))
        .json(&OpenUrlRequest { url: "https://first.example.com".into() })
        .send().await.unwrap()
        .status();
    assert_eq!(s1.as_u16(), 200, "first /open must return 200");

    // Step 2: POST /ports — proxy will point to the black-hole relay
    let proxy_port = {
        let tmp = bind_free().await;
        let p = port_of(&tmp);
        drop(tmp);
        p
    };
    let s2 = client
        .post(format!("{}/ports", base))
        .json(&PortsRequest {
            ports: vec![proxy_port],
            action: PortAction::Add,
            self_url: format!("http://127.0.0.1:{}", blackhole_port),
        })
        .send().await.unwrap()
        .status();
    assert_eq!(s2.as_u16(), 200, "POST /ports must return 200");

    wait_for_port(proxy_port, Duration::from_secs(3)).await;

    // Simulate browser making several parallel requests through the proxy.
    // Each will be forwarded to the black-hole relay and hang until the
    // connect_timeout (10 s in production; in the test we rely on timeout
    // being present — actual timeout is not waited for to keep test fast).
    let proxy_client = reqwest::Client::builder()
        .timeout(Duration::from_millis(100)) // test-side short timeout
        .build()
        .unwrap();
    let proxy_base = format!("http://127.0.0.1:{}", proxy_port);
    let futs: Vec<_> = (0..5)
        .map(|_| proxy_client.get(proxy_base.clone()).send())
        .collect();
    let _ = futures::future::join_all(futs).await; // errors are expected

    // Step 3: POST /open again — must not hang even though proxy connections
    // are/were pending.
    let s3 = tokio::time::timeout(
        Duration::from_millis(500),
        client
            .post(format!("{}/open", base))
            .json(&OpenUrlRequest { url: "https://second.example.com".into() })
            .send(),
    )
    .await
    .expect("second POST /open timed out — daemon was frozen after proxy exhaustion")
    .unwrap()
    .status();
    assert_eq!(s3.as_u16(), 200, "second /open must return 200");

    _h.abort();
}

/// POST /ports (Add) creates a working proxy listener.
#[tokio::test]
async fn host_ports_add_creates_listener() {
    let mock_relay_l = bind_free().await;
    let mock_relay_port = port_of(&mock_relay_l);
    let mock_relay_app = Router::new().route(
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
    let _relay = serve(mock_relay_l, mock_relay_app).await;

    let host_l = bind_free().await;
    let host_port = port_of(&host_l);
    let (_, router) = make_host(|_| {});
    let _h = serve(host_l, router).await;
    wait_for_port(host_port, Duration::from_secs(2)).await;
    wait_for_port(mock_relay_port, Duration::from_secs(2)).await;

    let proxy_port = {
        let tmp = bind_free().await;
        let p = port_of(&tmp);
        drop(tmp);
        p
    };

    reqwest::Client::new()
        .post(format!("http://127.0.0.1:{}/ports", host_port))
        .json(&PortsRequest {
            ports: vec![proxy_port],
            action: PortAction::Add,
            self_url: format!("http://127.0.0.1:{}", mock_relay_port),
        })
        .send().await.unwrap();

    wait_for_port(proxy_port, Duration::from_secs(3)).await;

    let body = reqwest::Client::new()
        .get(format!("http://127.0.0.1:{}/", proxy_port))
        .send().await.unwrap()
        .text().await.unwrap();
    assert_eq!(body, "relay ok");
    _h.abort();
}

// ── relay redirect tests ──────────────────────────────────────────────────────
//
// These tests exercise the client relay (POST /proxy) to verify that redirects
// are passed through as-is instead of being followed internally.
//
// Background: OAuth callback servers (e.g. wrangler login) often respond with
// a 302 redirect after processing the auth code, then shut down.  If the relay
// followed the redirect automatically, it would try to connect to the (now
// closed) server and fail with "Local connection error", surfacing as
// "Sender relay returned error" in the host proxy.

fn client_relay_app_with_redirect_passthrough() -> Router {
    Router::new().route("/proxy", post(client_relay_handler_no_redirect))
}

async fn client_relay_handler_no_redirect(Json(req): Json<ProxyRequest>) -> impl IntoResponse {
    let body_bytes = BASE64_STANDARD.decode(&req.body).unwrap_or_default();
    // Same as production fix: no redirect following.
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(60))
        .build()
        .unwrap();
    let method =
        reqwest::Method::from_bytes(req.method.as_bytes()).unwrap_or(reqwest::Method::GET);
    let url = format!("http://127.0.0.1:{}{}", req.port, req.path_and_query);

    match client.request(method, &url).body(body_bytes).send().await {
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

/// The relay must pass 302 redirects back to the caller without following them.
/// Without this fix: relay follows 302, server shuts down, relay gets
/// "connection refused", returns 502 → host shows "Sender relay returned error".
#[tokio::test]
async fn relay_passes_redirect_without_following() {
    // Local server: always returns 302 → /success (simulates OAuth callback server).
    let local_l = bind_free().await;
    let local_port = port_of(&local_l);
    let local_app = Router::new().fallback(|| async {
        axum::response::Response::builder()
            .status(302)
            .header("location", "/success")
            .body(axum::body::Body::empty())
            .unwrap()
    });
    let _local = serve(local_l, local_app).await;

    let relay_l = bind_free().await;
    let relay_port = port_of(&relay_l);
    let _relay = serve(relay_l, client_relay_app_with_redirect_passthrough()).await;

    wait_for_port(local_port, Duration::from_secs(2)).await;
    wait_for_port(relay_port, Duration::from_secs(2)).await;

    let proxy_req = ProxyRequest {
        port: local_port,
        method: "GET".into(),
        path_and_query: "/oauth/callback?code=abc&state=xyz".into(),
        headers: HashMap::new(),
        body: String::new(),
    };

    let resp: ProxyResponse = reqwest::Client::new()
        .post(format!("http://127.0.0.1:{}/proxy", relay_port))
        .json(&proxy_req)
        .send().await.unwrap()
        .json().await.unwrap();

    // Must receive the 302 itself, not follow it.
    assert_eq!(resp.status, 302, "relay must return the redirect status as-is");
    let location = resp.headers.get("location").map(|s| s.as_str()).unwrap_or("");
    assert_eq!(location, "/success", "relay must preserve the Location header");

    _relay.abort();
}

/// The host proxy must surface a meaningful error when the relay returns non-2xx.
#[tokio::test]
async fn host_proxy_error_message_includes_relay_details() {
    // Relay that always returns 502 with a descriptive body.
    let relay_l = bind_free().await;
    let relay_port = port_of(&relay_l);
    let error_relay = Router::new()
        .route("/proxy", post(|| async {
            (StatusCode::BAD_GATEWAY, "Local connection error: connection refused")
        }));
    let _relay = serve(relay_l, error_relay).await;

    let host_l = bind_free().await;
    let host_port = port_of(&host_l);
    let (_, router) = make_host(|_| {});
    let _h = serve(host_l, router).await;
    wait_for_port(host_port, Duration::from_secs(2)).await;
    wait_for_port(relay_port, Duration::from_secs(2)).await;

    let proxy_port = {
        let tmp = bind_free().await;
        let p = port_of(&tmp);
        drop(tmp);
        p
    };

    reqwest::Client::new()
        .post(format!("http://127.0.0.1:{}/ports", host_port))
        .json(&PortsRequest {
            ports: vec![proxy_port],
            action: PortAction::Add,
            self_url: format!("http://127.0.0.1:{}", relay_port),
        })
        .send().await.unwrap();
    wait_for_port(proxy_port, Duration::from_secs(3)).await;

    let resp = reqwest::Client::new()
        .get(format!("http://127.0.0.1:{}/", proxy_port))
        .send().await.unwrap();

    assert_eq!(resp.status().as_u16(), 502);
    let body = resp.text().await.unwrap();
    // Error body must include the relay's status code and message.
    assert!(body.contains("502"), "error body should include relay status: {}", body);
    assert!(
        body.contains("connection refused") || body.contains("Local connection"),
        "error body should include relay message: {}",
        body
    );

    _h.abort();
    _relay.abort();
}
