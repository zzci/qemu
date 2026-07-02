//! The single web endpoint: embedded UI, WebSocket↔unix-socket VNC/console bridges, power API.
//! VNC + console are QEMU unix sockets (no TCP listen, no VNC password) — this layer is the gate.

use crate::lifecycle::{Control, ControlTx};
use crate::log;
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Form, Path, Request, State,
    },
    http::{
        header::{CONTENT_TYPE, COOKIE, SET_COOKIE},
        HeaderMap, Method, StatusCode, Uri,
    },
    middleware::{self, Next},
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
    Json, Router,
};
use futures_util::{SinkExt, StreamExt};
use include_dir::{include_dir, Dir};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;

/// The built web console (ui/dist), embedded at compile time (build.rs guarantees the dir).
static UI: Dir<'static> = include_dir!("$CARGO_MANIFEST_DIR/ui/dist");

pub struct WebState {
    pub vnc_sock: PathBuf,        // {state}.vnc.sock  (QEMU VNC unix socket)
    pub console_sock: PathBuf,    // {state}.console.sock (may not exist if CONSOLE off)
    pub ctrl: ControlTx,          // power commands -> the supervisor
    pub allowed_origins: Vec<String>, // extra Origins to accept (beyond same-origin)
    pub password: String,         // access password ("" = auth disabled)
    pub token: String,            // random session secret handed out on a correct login
    pub info: VmInfo,             // static VM facts for /info (live status is queried per request)
}

/// A user-mode host→guest port forward (QEMU SLIRP `hostfwd` or a `PORT_FWD` entry).
#[derive(Clone, Serialize)]
pub struct PortForward {
    pub proto: String,
    pub host: u16,
    pub guest: u16,
}

/// Static VM facts computed once at startup; live run-state is queried per request.
#[derive(Clone, Serialize)]
pub struct VmInfo {
    pub name: String,
    pub accel: String,
    pub cpu: String,
    pub cpus: u32,
    pub ram: String,
    pub disk: String,
    pub disk_size: String,
    pub uuid: String,
    pub mac: String,
    pub tpm: bool,
    pub web_port: u16,
    pub port_forwards: Vec<PortForward>,
    pub command: String,
}

/// Collect host→guest forwards: `hostfwd=` segments in `scan` + a `PORT_FWD` "H-G,…" list, deduped.
pub fn collect_port_forwards(scan: &str, port_fwd_env: &str) -> Vec<PortForward> {
    let mut out: Vec<PortForward> = Vec::new();
    let mut push = |proto: String, host: u16, guest: u16| {
        if !out.iter().any(|p| p.proto == proto && p.host == host && p.guest == guest) {
            out.push(PortForward { proto, host, guest });
        }
    };
    // e.g. "hostfwd=tcp::3389-:3389" or "hostfwd=tcp:127.0.0.1:8080-:80" (a segment ends at , or space)
    for seg in scan.split("hostfwd=").skip(1) {
        let tok: String = seg.chars().take_while(|c| !c.is_whitespace() && *c != ',').collect();
        let Some((left, right)) = tok.split_once('-') else { continue };
        let proto = left.split(':').next().unwrap_or("tcp").to_string();
        let host = left.rsplit(':').next().and_then(|p| p.parse().ok());
        let guest = right.rsplit(':').next().and_then(|p| p.parse().ok());
        if let (Some(h), Some(g)) = (host, guest) {
            push(proto, h, g);
        }
    }
    // PORT_FWD = "3389-3389,2222-22" (tcp, host-guest)
    for pair in port_fwd_env.split(',') {
        let pair = pair.trim();
        if pair.is_empty() {
            continue;
        }
        if let Some((h, g)) = pair.split_once('-') {
            if let (Ok(h), Ok(g)) = (h.trim().parse(), g.trim().parse()) {
                push("tcp".into(), h, g);
            }
        }
    }
    out
}

/// CSRF/CSWSH guard: allow no Origin (CLI), same-origin (Host / X-Forwarded-Host) or an
/// explicitly allowed Origin; reject the rest.
fn origin_allowed(headers: &HeaderMap, allowed: &[String]) -> bool {
    let origin = match headers.get("origin").and_then(|v| v.to_str().ok()) {
        Some(o) => o,
        None => return true,
    };
    if allowed.iter().any(|a| a == origin) {
        return true;
    }
    let origin_host = origin.split_once("://").map(|(_, h)| h).unwrap_or(origin);
    let host = |k: &str| headers.get(k).and_then(|v| v.to_str().ok()).unwrap_or("");
    origin_host == host("host") || origin_host == host("x-forwarded-host")
}

// ---- access password ----------------------------------------------------------------------------
// `web.password` set: browsers log in (session cookie), programmatic clients send X-VMD-Password.
// Empty password = middleware is a no-op.

/// API/WS paths get 401 when unauthenticated (a login page would break them).
fn is_api(path: &str) -> bool {
    matches!(path, "/websockify" | "/console" | "/status" | "/info") || path.starts_with("/power")
}

/// Constant-time compare for password/token checks.
fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b) {
        diff |= x ^ y;
    }
    diff == 0
}

fn is_authed(req: &Request, s: &WebState) -> bool {
    let h = req.headers();
    // programmatic clients (e.g. the vmd CLI) present the password directly
    if let Some(p) = h.get("x-vmd-password").and_then(|v| v.to_str().ok()) {
        if ct_eq(p.as_bytes(), s.password.as_bytes()) {
            return true;
        }
    }
    // browsers present the session cookie handed out by /login
    let want = format!("vmd_auth={}", s.token);
    h.get(COOKIE)
        .and_then(|v| v.to_str().ok())
        .map(|c| c.split(';').any(|kv| kv.trim() == want))
        .unwrap_or(false)
}

/// Gate every route: login POST passes, browser navigation gets the login page, API/WS get 401.
async fn auth(State(s): State<Arc<WebState>>, req: Request, next: Next) -> Response {
    if s.password.is_empty() || is_authed(&req, &s) {
        return next.run(req).await;
    }
    if req.method() == Method::POST && req.uri().path() == "/login" {
        return next.run(req).await;
    }
    if req.method() == Method::GET && !is_api(req.uri().path()) {
        (StatusCode::OK, Html(login_html(false))).into_response()
    } else {
        (StatusCode::UNAUTHORIZED, "unauthorized\n").into_response()
    }
}

#[derive(Deserialize)]
struct Login {
    password: String,
}

async fn login(State(s): State<Arc<WebState>>, Form(f): Form<Login>) -> Response {
    if ct_eq(f.password.as_bytes(), s.password.as_bytes()) {
        let cookie = format!("vmd_auth={}; Path=/; HttpOnly; SameSite=Strict", s.token);
        ([(SET_COOKIE, cookie)], Redirect::to("/")).into_response()
    } else {
        (StatusCode::UNAUTHORIZED, Html(login_html(true))).into_response()
    }
}

fn login_html(error: bool) -> String {
    let err = if error { r#"<div class="err">Wrong password</div>"# } else { "" };
    format!(
        r#"<!doctype html><html lang="en"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1"><title>vmd — locked</title>
<style>body{{background:#0b0e14;color:#c9d1d9;font-family:system-ui,sans-serif;margin:0;
min-height:100vh;display:flex;align-items:center;justify-content:center}}
form{{background:#111722;padding:2rem;border-radius:10px;box-shadow:0 8px 30px #0008;width:260px}}
h1{{font-size:1rem;margin:0 0 1rem}}
input,button{{width:100%;box-sizing:border-box;padding:.6rem;border-radius:6px;font-size:1rem}}
input{{border:1px solid #30363d;background:#0b0e14;color:#c9d1d9;margin-bottom:.8rem}}
button{{border:0;background:#2f81f7;color:#fff;font-weight:600;cursor:pointer}}
.err{{color:#f85149;font-size:.85rem;margin-bottom:.6rem}}</style></head>
<body><form method="post" action="/login"><h1>&#128274; VM console locked</h1>{err}
<input type="password" name="password" placeholder="Password" autofocus autocomplete="current-password">
<button type="submit">Unlock</button></form></body></html>"#
    )
}

pub async fn serve(port: u16, state: WebState) {
    let state = Arc::new(state);
    let app = Router::new()
        .route("/websockify", get(vnc_ws))
        .route("/console", get(console_ws))
        .route("/power/:action", post(power))
        .route("/status", get(status))
        .route("/info", get(info))
        .route("/login", post(login))
        .fallback(static_asset)
        .layer(middleware::from_fn_with_state(state.clone(), auth))
        .with_state(state);

    match tokio::net::TcpListener::bind(("0.0.0.0", port)).await {
        Ok(listener) => {
            log::info(format!("web console on :{port} (noVNC /vnc.html, WS /websockify /console)"));
            if let Err(e) = axum::serve(listener, app).await {
                log::error(format!("web server: {e}"));
            }
        }
        Err(e) => log::error(format!("web bind :{port}: {e}")),
    }
}

// ---- VNC + console: browser WebSocket <-> QEMU unix socket ---------------------------------------

async fn vnc_ws(ws: WebSocketUpgrade, headers: HeaderMap, State(s): State<Arc<WebState>>) -> Response {
    bridge(ws, &headers, &s, s.vnc_sock.clone(), "VNC")
}

async fn console_ws(ws: WebSocketUpgrade, headers: HeaderMap, State(s): State<Arc<WebState>>) -> Response {
    bridge(ws, &headers, &s, s.console_sock.clone(), "console")
}

/// Origin-guarded raw byte pump between the browser WebSocket and a QEMU unix socket.
fn bridge(ws: WebSocketUpgrade, headers: &HeaderMap, s: &WebState, sock: PathBuf, label: &'static str) -> Response {
    if !origin_allowed(headers, &s.allowed_origins) {
        return (StatusCode::FORBIDDEN, "origin not allowed\n").into_response();
    }
    ws.on_upgrade(move |socket| async move {
        match UnixStream::connect(&sock).await {
            Ok(unix) => pump_unix(socket, unix).await,
            Err(e) => log::warn(format!("{label} connect {}: {e}", sock.display())),
        }
    })
    .into_response()
}

async fn pump_unix(ws: WebSocket, unix: UnixStream) {
    let (mut ur, mut uw) = unix.into_split();
    let (mut wtx, mut wrx) = ws.split();
    let to_unix = async {
        while let Some(Ok(msg)) = wrx.next().await {
            let bytes = match msg {
                Message::Binary(b) => b,
                Message::Text(t) => t.into_bytes(),
                Message::Close(_) => break,
                _ => continue,
            };
            if uw.write_all(&bytes).await.is_err() {
                break;
            }
        }
    };
    let to_ws = async {
        let mut buf = vec![0u8; 65536];
        loop {
            match ur.read(&mut buf).await {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    if wtx.send(Message::Binary(buf[..n].to_vec())).await.is_err() {
                        break;
                    }
                }
            }
        }
    };
    tokio::select! { _ = to_unix => {}, _ = to_ws => {} }
}

// ---- power API ----------------------------------------------------------------------------------

async fn power(
    Path(action): Path<String>,
    headers: HeaderMap,
    State(s): State<Arc<WebState>>,
) -> impl IntoResponse {
    if !origin_allowed(&headers, &s.allowed_origins) {
        return (StatusCode::FORBIDDEN, "origin not allowed\n").into_response();
    }
    let cmd = match action.as_str() {
        "start" => Control::Start,
        "shutdown" => Control::Shutdown,
        "reset" => Control::Reset,
        "poweroff" => Control::PowerOff,
        _ => {
            return (StatusCode::BAD_REQUEST, "action: start|shutdown|reset|poweroff\n").into_response()
        }
    };
    match s.ctrl.send(cmd).await {
        Ok(()) => (StatusCode::OK, format!("{action}\n")).into_response(),
        Err(_) => (StatusCode::SERVICE_UNAVAILABLE, "vm not running\n").into_response(),
    }
}

/// Live run-state from the supervisor; `None` = supervisor gone.
async fn query_status(s: &WebState) -> Option<String> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    s.ctrl.send(Control::Status(tx)).await.ok()?;
    rx.await.ok()
}

async fn status(State(s): State<Arc<WebState>>) -> impl IntoResponse {
    match query_status(&s).await {
        Some(state) => (StatusCode::OK, format!("{state}\n")).into_response(),
        None => (StatusCode::SERVICE_UNAVAILABLE, "vm not running\n").into_response(),
    }
}

// ---- embedded web console (SPA) -----------------------------------------------------------------

/// Serve embedded ui/dist; unknown paths fall back to index.html (SPA).
async fn static_asset(uri: Uri) -> Response {
    let raw = uri.path().trim_start_matches('/');
    let path = if raw.is_empty() { "index.html" } else { raw };
    match UI.get_file(path).or_else(|| UI.get_file("index.html")) {
        Some(f) => {
            let ct = content_type(f.path().extension().and_then(|e| e.to_str()));
            ([(CONTENT_TYPE, ct)], f.contents()).into_response()
        }
        None => (StatusCode::NOT_FOUND, "not found\n").into_response(),
    }
}

fn content_type(ext: Option<&str>) -> &'static str {
    match ext.unwrap_or("") {
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" | "map" => "application/json; charset=utf-8",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "ico" => "image/x-icon",
        "webp" => "image/webp",
        "woff2" => "font/woff2",
        "woff" => "font/woff",
        "ttf" => "font/ttf",
        "wasm" => "application/wasm",
        "txt" => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::collect_port_forwards;

    #[test]
    fn parses_hostfwd_from_command() {
        let cmd = "-netdev user,id=net0,hostfwd=tcp::3389-:3389 -device virtio-net-pci";
        let fwds = collect_port_forwards(cmd, "");
        assert_eq!(fwds.len(), 1);
        assert_eq!(fwds[0].proto, "tcp");
        assert_eq!((fwds[0].host, fwds[0].guest), (3389, 3389));
    }

    #[test]
    fn parses_hostfwd_with_bind_address() {
        let fwds = collect_port_forwards("hostfwd=tcp:127.0.0.1:8080-:80", "");
        assert_eq!((fwds[0].host, fwds[0].guest), (8080, 80));
    }

    #[test]
    fn parses_port_fwd_env_and_dedups() {
        // one forward only in the command, one only in PORT_FWD, one in both (deduped)
        let fwds = collect_port_forwards("hostfwd=tcp::3389-:3389", "3389-3389,2222-22");
        assert_eq!(fwds.len(), 2);
        assert!(fwds.iter().any(|p| (p.host, p.guest) == (3389, 3389)));
        assert!(fwds.iter().any(|p| (p.host, p.guest) == (2222, 22)));
    }

    #[test]
    fn empty_when_no_forwards() {
        assert!(collect_port_forwards("-nic none", "").is_empty());
    }
}

/// Home-screen payload: live status + static VM facts.
async fn info(State(s): State<Arc<WebState>>) -> impl IntoResponse {
    #[derive(Serialize)]
    struct InfoResponse {
        status: String,
        #[serde(flatten)]
        info: VmInfo,
    }
    let status = query_status(&s).await.unwrap_or_else(|| "unknown".into());
    Json(InfoResponse { status, info: s.info.clone() })
}
