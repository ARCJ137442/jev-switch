//! ListenSupervisor / mode 热切 —— **真实 TCP** 集成护栏（零进程重启铁证组）。
//!
//! 与 `integration_http.rs`（oneshot、不绑端口）分工：本文件全程经
//! `listen::start` 起真实 listener + reqwest 客户端，覆盖：
//! 1. local 态非 loopback peer → 403（显式 bind=0.0.0.0 的 peer 兜底）
//! 2. `PUT /listen` 热 Rebind：旧端口 refused、新端口服务、`auto` 改回、bind 落盘
//! 3. mode 翻转联动 Rebind（成对默认同端口 127.0.0.1 ⇄ 0.0.0.0 —— 阶段 2 交接）：
//!    LAN 从 refused → 可达+401 → 切回 refused；同请求 200→401→200 即时翻转
//! 4. try-bind 失败保旧（目标端口被占 → 500，旧端口继续服务，文件不写）
//! 5. 在途请求不中断（慢上游请求中途 rebind → 仍 200 完成）
//! 6. `PUT /password` 真实 TCP 矩阵：cloud+会话 ✅ / cloud+LAN 无会话 ❌401 /
//!    cloud+loopback 无会话 ✅ / 改密后旧 session 401
//!
//! 全程只用**临时端口**（ephemeral），绝不碰 11435（用户本机 daemon 在跑）。
//! LAN 探测用本机局域网 IP（UDP connect 技巧取网卡地址，不发包）；环境无
//! 局域网地址时相关断言段提前返回（本机开发环境已实跑记录于交付报告）。

use axum::body::Body;
use axum::http::Request;
use http_body_util::BodyExt;
use jev_core::adapter::{Registry, UpstreamAdapter};
use jev_core::router::RouteEdge;
use jev_core::upstream::{Capabilities, JevError, QuestionType};
use jev_protocol::{JevRequest, JevResponse};
use jev_switch_daemon::config::Config;
use jev_switch_daemon::listen::{self, ListenHandle, ListenPlan, PairDefaults};
use jev_switch_daemon::{build_app, build_state, AppState};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tower::ServiceExt;

/// 测试假凭据（真实值永不进测试/提交）。
const FAKE_TOK: &str = "tok-test-rebind-aaaa";
const FAKE_PW: &str = "pw-test-rebind-1";

/* ══════════════════════════════════════════════════════════════════
   夹具
   ══════════════════════════════════════════════════════════════════ */

fn temp_config(name: &str, content: &str) -> PathBuf {
    static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let seq = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "jev-rebind-{}-{}-{}",
        name,
        std::process::id(),
        seq
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("providers.toml");
    std::fs::write(&path, content).unwrap();
    path
}

fn cleanup(path: &std::path::Path) {
    if let Some(dir) = path.parent() {
        let _ = std::fs::remove_dir_all(dir);
    }
}

/// 临时端口（bind :0 取号即放；测试间碰撞概率可忽略）。
fn ephemeral_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

/// 本机局域网 IPv4（UDP connect 不发包，仅取内核选出的出口源地址）。
fn lan_ip() -> Option<std::net::Ipv4Addr> {
    let s = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    s.connect("8.8.8.8:80").ok()?;
    match s.local_addr().ok()?.ip() {
        std::net::IpAddr::V4(v4) if !v4.is_loopback() && !v4.is_unspecified() => Some(v4),
        _ => None,
    }
}

/// 起真实服务：temp config → state → app → ListenSupervisor。
async fn start_server(
    name: &str,
    cfg_content: &str,
    plan: ListenPlan,
) -> (AppState, ListenHandle, PathBuf) {
    let path = temp_config(name, cfg_content);
    let cfg = Config::load(&path).expect("load temp config");
    let state = build_state(cfg, path.clone());
    let app = build_app(state.clone());
    let handle = listen::start(app, plan, &state.listen)
        .await
        .expect("start listen");
    (state, handle, path)
}

/// 携自定义 Registry 起服务（慢上游等 fake 用）。
async fn start_server_with_registry(
    name: &str,
    registry: Registry,
    cfg_content: &str,
    plan: ListenPlan,
) -> (AppState, ListenHandle, PathBuf) {
    let path = temp_config(name, cfg_content);
    let mut state = build_state(Config::load(&path).unwrap(), path.clone());
    // 换成测试 Registry（AppState 字段 pub，直接替换 Arc）
    state.registry = Arc::new(registry);
    let app = build_app(state.clone());
    let handle = listen::start(app, plan, &state.listen)
        .await
        .expect("start listen");
    (state, handle, path)
}

async fn http_get(base: &str, path: &str, bearer: Option<&str>) -> Result<(u16, String), String> {
    let client = reqwest::Client::new();
    let mut req = client.get(format!("{base}{path}"));
    if let Some(t) = bearer {
        req = req.header("authorization", format!("Bearer {t}"));
    }
    match req.send().await {
        Ok(resp) => {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            Ok((status, body))
        }
        Err(e) => Err(format!("transport error: {e}")),
    }
}

async fn http_put(
    base: &str,
    path: &str,
    json: serde_json::Value,
    bearer: Option<&str>,
) -> Result<(u16, String), String> {
    let client = reqwest::Client::new();
    let mut req = client
        .put(format!("{base}{path}"))
        .header("content-type", "application/json")
        .body(serde_json::to_string(&json).unwrap());
    if let Some(t) = bearer {
        req = req.header("authorization", format!("Bearer {t}"));
    }
    match req.send().await {
        Ok(resp) => {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            Ok((status, body))
        }
        Err(e) => Err(format!("transport error: {e}")),
    }
}

async fn http_post(
    base: &str,
    path: &str,
    json: serde_json::Value,
) -> Result<(u16, String), String> {
    let client = reqwest::Client::new();
    match client
        .post(format!("{base}{path}"))
        .header("content-type", "application/json")
        .body(serde_json::to_string(&json).unwrap())
        .send()
        .await
    {
        Ok(resp) => {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            Ok((status, body))
        }
        Err(e) => Err(format!("transport error: {e}")),
    }
}

/// 断言连接被拒（热切后旧端口必须 refused —— 而非超时/404）。
async fn assert_refused(base: &str, path: &str, what: &str) {
    match http_get(base, path, None).await {
        Ok((s, b)) => panic!("{what}: 期望连接拒绝，却拿到 {s}: {b}"),
        Err(e) => assert!(e.contains("error"), "{what}: 期望 transport error，得到 {e}"),
    }
}

/// 最小 local 配置（无密码、带调用 token）。
fn local_cfg() -> String {
    format!(
        r#"
mode = "local"
auth_tokens = ["{FAKE_TOK}"]
[providers.laya]
kind = "laya"
base = "http://127.0.0.1:1/v1/systemone"
enabled = true

[router]
"laya-english" = "laya"
"#
    )
}

/// 带密码的 local 配置（激活 cloud 的既有密码场景）。
fn local_with_pw_cfg() -> String {
    format!(
        r#"
mode = "local"
auth_tokens = ["{FAKE_TOK}"]
admin_password = "{FAKE_PW}"
[providers.laya]
kind = "laya"
base = "http://127.0.0.1:1/v1/systemone"
enabled = true

[router]
"laya-english" = "laya"
"#
    )
}

/// 同端口成对默认对（真实语义：local 127.0.0.1:P ⇄ cloud 0.0.0.0:P）。
fn same_port_pair(port: u16) -> PairDefaults {
    PairDefaults {
        local: std::net::SocketAddr::from(([127, 0, 0, 1], port)),
        cloud: std::net::SocketAddr::from(([0, 0, 0, 0], port)),
    }
}

/* ══════════════════════════════════════════════════════════════════
   ① local 态非 loopback peer → 403（显式 bind 兜底 —— 方案二残留保护）
   ══════════════════════════════════════════════════════════════════ */

#[tokio::test]
async fn local_mode_non_loopback_peer_is_403_over_real_tcp() {
    let Some(lan) = lan_ip() else {
        eprintln!("SKIP（本环境无局域网 IP，未执行 LAN 403 断言）");
        return;
    };
    let port = ephemeral_port();
    // 显式 bind 0.0.0.0 + local —— LAN 可达但应用层 peer 校验必须 403
    let plan = ListenPlan {
        addr: std::net::SocketAddr::from(([0, 0, 0, 0], port)),
        explicit: true,
        defaults: standard_plan_addr(),
    };
    // 配置里真的写 bind 键（显式绑定的真值源；与 plan 一致）
    let cfg = format!("bind = \"0.0.0.0:{port}\"\n{}", local_with_pw_cfg());
    let (_state, handle, path) = start_server("peer403", &cfg, plan).await;
    let lan_base = format!("http://{lan}:{port}");
    let loop_base = format!("http://127.0.0.1:{port}");

    // loopback → 200（local 现状语义）
    let (s, b) = http_get(&loop_base, "/v1/models", None).await.unwrap();
    assert_eq!(s, 200, "loopback local 免 token: {b}");

    // LAN → 403 + 三键 + 文案（真实 TCP peer = 局域网 IP）
    let (s, b) = http_get(&lan_base, "/v1/models", None).await.unwrap();
    assert_eq!(s, 403, "local 非 loopback 必须 403: {b}");
    let v: serde_json::Value = serde_json::from_str(&b).unwrap();
    let obj = v.as_object().expect("403 是对象");
    assert_eq!(obj.len(), 3, "错误体恰三键: {b}");
    assert!(v["error"].as_str().unwrap().contains("local mode: loopback only"), "{b}");
    assert_eq!(v["upstream"], serde_json::Value::Null, "{b}");
    assert_eq!(v["retryable"], false, "{b}");

    // admin 域同样 403（peer 兜底先于一切）
    let (s, b) = http_get(&lan_base, "/v1/admin/providers", None).await.unwrap();
    assert_eq!(s, 403, "admin LAN 也要 403: {b}");
    // health 探活不受 peer 限制（公开门）
    let (s, b) = http_get(&lan_base, "/health", None).await.unwrap();
    assert_eq!(s, 200, "health 双态放行: {b}");

    // 显式 bind：mode 翻转不 Rebind（响应 skipped + bound 不变）
    let before = handle.bound();
    let (s, b) = http_put(
        &loop_base,
        "/v1/admin/mode",
        serde_json::json!({"mode":"cloud"}),
        None,
    )
    .await
    .unwrap();
    assert_eq!(s, 200, "{b}");
    let v: serde_json::Value = serde_json::from_str(&b).unwrap();
    assert_eq!(v["rebind"]["skipped"], "explicit bind", "{b}");
    assert_eq!(handle.bound(), before, "显式 bind 地址不动");
    let on_disk = std::fs::read_to_string(&path).unwrap();
    assert!(on_disk.contains("bind = "), "显式 bind 应在配置中: {on_disk}");
    cleanup(&path);
}

/// standard_plan 的 defaults 引用（0.0.0.0:11435 不会真用到 —— 仅占位；
/// 本测试 plan 已显式给出 defaults）。收敛成小助手避免重复。
fn standard_plan_addr() -> PairDefaults {
    PairDefaults::standard()
}

/* ══════════════════════════════════════════════════════════════════
   ② PUT /listen 热 Rebind：旧端口 refused、新端口 200、auto 改回、bind 落盘
   ══════════════════════════════════════════════════════════════════ */

#[tokio::test]
async fn put_listen_hot_rebind_moves_port_without_restart() {
    let port_a = ephemeral_port();
    let port_b = ephemeral_port();
    let addr_a: std::net::SocketAddr = format!("127.0.0.1:{port_a}").parse().unwrap();
    // 成对默认锚定 A（auto 改回目标）；初始 plan 非显式
    let plan = ListenPlan {
        addr: addr_a,
        explicit: false,
        defaults: PairDefaults {
            local: addr_a,
            cloud: addr_a,
        },
    };
    let (_state, handle, path) = start_server("hotport", &local_cfg(), plan).await;
    let base_a = format!("http://127.0.0.1:{port_a}");
    let base_b = format!("http://127.0.0.1:{port_b}");

    // A 正常服务
    let (s, b) = http_get(&base_a, "/health", None).await.unwrap();
    assert_eq!(s, 200, "{b}");

    // 热切到 B（显式化）
    let (s, b) = http_put(
        &base_a,
        "/v1/admin/listen",
        serde_json::json!({"addr": format!("127.0.0.1:{port_b}")}),
        None,
    )
    .await
    .unwrap();
    assert_eq!(s, 200, "rebind 应成功: {b}");
    let v: serde_json::Value = serde_json::from_str(&b).unwrap();
    assert_eq!(v["addr"], format!("127.0.0.1:{port_b}"));
    assert_eq!(v["rebound"], true, "{b}");

    // 旧端口 refused、新端口 200 —— **同一进程**（handle/state 未变）
    assert_refused(&base_a, "/health", "旧端口 A").await;
    let (s, b) = http_get(&base_b, "/health", None).await.unwrap();
    assert_eq!(s, 200, "新端口 B 服务: {b}");
    assert_eq!(handle.bound().port(), port_b);
    // GET listen 查询
    let (s, b) = http_get(&base_b, "/v1/admin/listen", None).await.unwrap();
    assert_eq!(s, 200, "{b}");
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&b).unwrap()["addr"],
        format!("127.0.0.1:{port_b}")
    );
    // status.bind 实时反映 Rebind 后地址 + bind_explicit 已翻 true
    let (s, b) = http_get(&base_b, "/v1/admin/status", None).await.unwrap();
    assert_eq!(s, 200, "{b}");
    let v: serde_json::Value = serde_json::from_str(&b).unwrap();
    assert_eq!(v["bind"], format!("127.0.0.1:{port_b}"), "status.bind 实时: {b}");
    assert_eq!(v["bind_explicit"], true, "PUT listen 显式化后: {b}");
    assert_eq!(v["mode"], "local", "{b}");
    // bind 显式落盘
    let on_disk = std::fs::read_to_string(&path).unwrap();
    assert!(
        on_disk.contains(&format!("bind = \"127.0.0.1:{port_b}\"")),
        "{on_disk}"
    );

    // addr=auto → 改回成对默认（本 plan = A），bind 键移除
    let (s, b) = http_put(
        &base_b,
        "/v1/admin/listen",
        serde_json::json!({"addr":"auto"}),
        None,
    )
    .await
    .unwrap();
    assert_eq!(s, 200, "{b}");
    assert_refused(&base_b, "/health", "B 在 auto 后").await;
    let (s, b) = http_get(&base_a, "/health", None).await.unwrap();
    assert_eq!(s, 200, "auto 改回 A: {b}");
    let on_disk = std::fs::read_to_string(&path).unwrap();
    assert!(!on_disk.contains("bind = "), "auto 应移除 bind 键: {on_disk}");
    assert!(!handle.is_explicit(), "auto 后回到非显式");
    // status 回到 A + 显式标志复位
    let (s, b) = http_get(&base_a, "/v1/admin/status", None).await.unwrap();
    assert_eq!(s, 200, "{b}");
    let v: serde_json::Value = serde_json::from_str(&b).unwrap();
    assert_eq!(v["bind"], format!("127.0.0.1:{port_a}"), "{b}");
    assert_eq!(v["bind_explicit"], false, "{b}");
    cleanup(&path);
}

/* ══════════════════════════════════════════════════════════════════
   ③ mode 翻转联动 Rebind（同端口成对默认 —— 阶段 2 优雅交接）+ 即时鉴权翻转
   ══════════════════════════════════════════════════════════════════ */

#[tokio::test]
async fn mode_flip_triggers_rebind_and_instant_auth_flip_real_tcp() {
    let Some(lan) = lan_ip() else {
        eprintln!("SKIP（本环境无局域网 IP，未执行 LAN 可达性断言）");
        return;
    };
    let port = ephemeral_port();
    let plan = ListenPlan {
        addr: std::net::SocketAddr::from(([127, 0, 0, 1], port)),
        explicit: false, // 非显式 → mode 翻转必须联动 Rebind
        defaults: same_port_pair(port),
    };
    let (_state, handle, path) = start_server("modeflip", &local_with_pw_cfg(), plan).await;
    let loop_base = format!("http://127.0.0.1:{port}");
    let lan_base = format!("http://{lan}:{port}");

    // local：LAN refused（内核级关闭 127.0.0.1 绑定）；loopback 免 token 200
    assert_refused(&lan_base, "/health", "local 态 LAN").await;
    let (s, b) = http_get(&loop_base, "/v1/models", None).await.unwrap();
    assert_eq!(s, 200, "切前 local: {b}");

    // local → cloud（loopback 直达 mode 端点；config 已有密码 → 无需带）
    let (s, b) = http_put(
        &loop_base,
        "/v1/admin/mode",
        serde_json::json!({"mode":"cloud"}),
        None,
    )
    .await
    .unwrap();
    assert_eq!(s, 200, "{b}");
    let v: serde_json::Value = serde_json::from_str(&b).unwrap();
    assert_eq!(v["mode"], "cloud");
    assert_eq!(v["persisted"], true);
    assert_eq!(v["rebind"]["ok"], true, "非显式必须 Rebind: {b}");
    assert_eq!(
        v["rebind"]["to"],
        format!("0.0.0.0:{port}"),
        "cloud 成对默认: {b}"
    );
    assert_eq!(handle.bound().to_string(), format!("0.0.0.0:{port}"));

    // LAN 现在可达 + 401（cloud 鉴权；from refused → reachable 是方案一铁证）
    let (s, b) = http_get(&lan_base, "/v1/models", None).await.unwrap();
    assert_eq!(s, 401, "cloud LAN 可达且要 token: {b}");
    // 同请求即时翻转：loopback 无 token 401 / 带 token 200
    let (s, b) = http_get(&loop_base, "/v1/models", None).await.unwrap();
    assert_eq!(s, 401, "切后 401: {b}");
    let (s, b) = http_get(&loop_base, "/v1/models", Some(FAKE_TOK)).await.unwrap();
    assert_eq!(s, 200, "{b}");

    // cloud → local：需 admin 会话（防匿名拆锁）
    let (s, b) = http_put(
        &loop_base,
        "/v1/admin/mode",
        serde_json::json!({"mode":"local"}),
        None,
    )
    .await
    .unwrap();
    assert_eq!(s, 401, "cloud 翻转无会话必须 401: {b}");
    let (s, b) = http_post(
        &loop_base,
        "/v1/admin/login",
        serde_json::json!({"password": FAKE_PW}),
    )
    .await
    .unwrap();
    assert_eq!(s, 200, "{b}");
    let session = serde_json::from_str::<serde_json::Value>(&b).unwrap()["token"]
        .as_str()
        .unwrap()
        .to_string();
    let (s, b) = http_put(
        &loop_base,
        "/v1/admin/mode",
        serde_json::json!({"mode":"local"}),
        Some(&session),
    )
    .await
    .unwrap();
    assert_eq!(s, 200, "{b}");
    let v: serde_json::Value = serde_json::from_str(&b).unwrap();
    assert_eq!(v["rebind"]["ok"], true, "{b}");
    assert_eq!(
        v["rebind"]["to"],
        format!("127.0.0.1:{port}"),
        "local 成对默认: {b}"
    );
    assert_eq!(handle.bound().to_string(), format!("127.0.0.1:{port}"));

    // 切回：LAN 再次 refused；loopback 无 token 恢复 200
    assert_refused(&lan_base, "/health", "local 态 LAN（切回后）").await;
    let (s, b) = http_get(&loop_base, "/v1/models", None).await.unwrap();
    assert_eq!(s, 200, "切回 local 即时恢复: {b}");
    let on_disk = std::fs::read_to_string(&path).unwrap();
    assert!(on_disk.contains("mode = \"local\""), "{on_disk}");
    cleanup(&path);
}

/* ══════════════════════════════════════════════════════════════════
   ④ try-bind 失败保旧：占用目标端口 → 500 + 旧端口继续服务 + 文件不写
   ══════════════════════════════════════════════════════════════════ */

#[tokio::test]
async fn try_bind_failure_keeps_old_listener_and_config() {
    let port_a = ephemeral_port();
    let port_busy = ephemeral_port();
    let addr_a: std::net::SocketAddr = format!("127.0.0.1:{port_a}").parse().unwrap();
    let plan = ListenPlan {
        addr: addr_a,
        explicit: false,
        defaults: PairDefaults {
            local: addr_a,
            cloud: addr_a,
        },
    };
    let (_state, _handle, path) = start_server("bindfail", &local_cfg(), plan).await;
    let base_a = format!("http://127.0.0.1:{port_a}");

    // 第三方占住目标端口
    let squatter = std::net::TcpListener::bind(("127.0.0.1", port_busy)).unwrap();

    let (s, b) = http_put(
        &base_a,
        "/v1/admin/listen",
        serde_json::json!({"addr": format!("127.0.0.1:{port_busy}")}),
        None,
    )
    .await
    .unwrap();
    assert_eq!(s, 500, "try-bind 失败 → 500（带病不上线）: {b}");
    let v: serde_json::Value = serde_json::from_str(&b).unwrap();
    assert!(v["error"].as_str().unwrap().contains("rebind failed"), "{b}");
    assert_eq!(v.as_object().unwrap().len(), 3, "错误体三键: {b}");

    // 旧端口继续服务（一字未动）；文件未写 bind
    let (s, b) = http_get(&base_a, "/health", None).await.unwrap();
    assert_eq!(s, 200, "失败后旧监听必须健在: {b}");
    let on_disk = std::fs::read_to_string(&path).unwrap();
    assert!(
        !on_disk.contains(&format!("127.0.0.1:{port_busy}")),
        "失败不得落盘: {on_disk}"
    );
    drop(squatter);

    // 非法地址字符串 → 400
    let (s, b) = http_put(
        &base_a,
        "/v1/admin/listen",
        serde_json::json!({"addr":"not-an-addr"}),
        None,
    )
    .await
    .unwrap();
    assert_eq!(s, 400, "{b}");
    cleanup(&path);
}

/* ══════════════════════════════════════════════════════════════════
   ⑤ 在途请求不中断：慢上游请求中途 Rebind → 仍 200 完成
   ══════════════════════════════════════════════════════════════════ */

/// 慢 fake 上游（sleep 600ms 后标准成功响应）。
struct SlowFake {
    delay: Duration,
}

#[async_trait::async_trait]
impl UpstreamAdapter for SlowFake {
    fn id(&self) -> &str {
        "slow"
    }
    fn capabilities(&self) -> Capabilities {
        Capabilities {
            question_types: &[QuestionType::Noul],
            has_confidence: false,
            has_usage: false,
            noul_via_boolean: false,
            retryable_status: &[408, 429, 500, 502, 503, 504],
        }
    }
    async fn evaluate(&self, _req: JevRequest) -> Result<JevResponse, JevError> {
        tokio::time::sleep(self.delay).await;
        serde_json::from_str(r#"{"answers":{}}"#).map_err(|e| JevError::BadResponse {
            upstream_id: "slow".into(),
            message: e.to_string(),
        })
    }
}

#[tokio::test]
async fn in_flight_request_survives_rebind() {
    let port_a = ephemeral_port();
    let port_b = ephemeral_port();
    let addr_a: std::net::SocketAddr = format!("127.0.0.1:{port_a}").parse().unwrap();

    let mut reg = Registry::new(vec![RouteEdge {
        left: "jev".into(),
        r#match: Default::default(),
        right: "slow".into(),
        upstream_model: None,
        priority: 10,
        sticky: Default::default(),
        on_error: Default::default(),
    }]);
    reg.register(Box::new(SlowFake {
        delay: Duration::from_millis(600),
    }));

    let plan = ListenPlan {
        addr: addr_a,
        explicit: false,
        defaults: PairDefaults {
            local: addr_a,
            cloud: addr_a,
        },
    };
    let (_state, _handle, path) =
        start_server_with_registry("inflight", reg, &local_cfg(), plan).await;
    let base_a = format!("http://127.0.0.1:{port_a}");
    let base_b = format!("http://127.0.0.1:{port_b}");

    // 发一个要跑 ~600ms 的 systemone
    let body = serde_json::json!({
        "model": "jev",
        "state": "hi",
        "questions": {
            "q": {"type":"noul","instructions":"x","criteria":{"true":"y","false":"n"}}
        }
    });
    let in_flight = tokio::spawn({
        let base = base_a.clone();
        async move { http_post(&base, "/v1/systemone", body).await }
    });

    // 请求在途（150ms 后）执行 Rebind —— 旧 serve 停 accept 但必须等在途收尾
    tokio::time::sleep(Duration::from_millis(150)).await;
    let (s, b) = http_put(
        &base_a,
        "/v1/admin/listen",
        serde_json::json!({"addr": format!("127.0.0.1:{port_b}")}),
        None,
    )
    .await
    .unwrap();
    assert_eq!(s, 200, "rebind 本身成功: {b}");

    // 在途请求仍 200 完成（不因换监听被掐）
    let result = in_flight.await.expect("join in-flight").expect("transport");
    assert_eq!(result.0, 200, "在途请求必须完整跑完: {}", result.1);

    // 新端口接管
    let (s, b) = http_get(&base_b, "/health", None).await.unwrap();
    assert_eq!(s, 200, "rebind 后新端口: {b}");
    cleanup(&path);
}

/* ══════════════════════════════════════════════════════════════════
   ⑥ PUT /password 真实 TCP 矩阵（cloud：会话 / LAN 无会话 / loopback 无会话）
   ══════════════════════════════════════════════════════════════════ */

#[tokio::test]
async fn put_password_real_tcp_auth_matrix() {
    let Some(lan) = lan_ip() else {
        eprintln!("SKIP（本环境无局域网 IP，未执行 LAN 401 断言）");
        return;
    };
    let port = ephemeral_port();
    let cfg = format!(
        r#"
mode = "cloud"
auth_tokens = ["{FAKE_TOK}"]
admin_password = "{FAKE_PW}"
[providers.laya]
kind = "laya"
base = "http://127.0.0.1:1/v1/systemone"
enabled = true

[router]
"laya-english" = "laya"
"#
    );
    let plan = ListenPlan {
        addr: std::net::SocketAddr::from(([0, 0, 0, 0], port)),
        explicit: true, // 钉住 0.0.0.0（本测只关心鉴权矩阵，不动监听）
        defaults: standard_plan_addr(),
    };
    let (_state, _handle, path) = start_server("pwdmatrix", &cfg, plan).await;
    let loop_base = format!("http://127.0.0.1:{port}");
    let lan_base = format!("http://{lan}:{port}");

    // 会话（loopback login）
    let (s, b) = http_post(
        &loop_base,
        "/v1/admin/login",
        serde_json::json!({"password": FAKE_PW}),
    )
    .await
    .unwrap();
    assert_eq!(s, 200, "{b}");
    let session = serde_json::from_str::<serde_json::Value>(&b).unwrap()["token"]
        .as_str()
        .unwrap()
        .to_string();

    // ① cloud + 有效会话（LAN）→ 200（轮换到 pw2，旧会话随之作废）
    let (s, b) = http_put(
        &lan_base,
        "/v1/admin/password",
        serde_json::json!({"password":"pw-test-matrix-2"}),
        Some(&session),
    )
    .await
    .unwrap();
    assert_eq!(s, 200, "cloud+会话（LAN）放行: {b}");

    // ② 旧 session 已作废 —— LAN 无会话 → 401；带死会话也 401
    let (s, b) = http_put(
        &lan_base,
        "/v1/admin/password",
        serde_json::json!({"password":"pw-test-matrix-x"}),
        None,
    )
    .await
    .unwrap();
    assert_eq!(s, 401, "cloud+LAN 无会话必须 401: {b}");
    let (s, b) = http_put(
        &lan_base,
        "/v1/admin/password",
        serde_json::json!({"password":"pw-test-matrix-x"}),
        Some(&session),
    )
    .await
    .unwrap();
    assert_eq!(s, 401, "死会话 401: {b}");

    // ③ cloud + loopback 无会话 → 200（忘密恢复语义；轮换到 pw3）
    let (s, b) = http_put(
        &loop_base,
        "/v1/admin/password",
        serde_json::json!({"password":"pw-test-matrix-3"}),
        None,
    )
    .await
    .unwrap();
    assert_eq!(s, 200, "cloud+loopback 无会话放行: {b}");
    let v: serde_json::Value = serde_json::from_str(&b).unwrap();
    assert_eq!(v["updated"], true);
    assert_eq!(v["env_override_active"], false, "{b}");

    // ④ 密码代际：pw3 登录 ✅、pw2/pw1 登录 ❌、pw2 会话（若再造）也已全废
    let (s, b) = http_post(
        &loop_base,
        "/v1/admin/login",
        serde_json::json!({"password":"pw-test-matrix-3"}),
    )
    .await
    .unwrap();
    assert_eq!(s, 200, "最终密码登录: {b}");
    let (s, _) = http_post(
        &loop_base,
        "/v1/admin/login",
        serde_json::json!({"password":"pw-test-matrix-2"}),
    )
    .await
    .unwrap();
    assert_eq!(s, 401, "上一手密码 401");
    let (s, _) = http_post(
        &loop_base,
        "/v1/admin/login",
        serde_json::json!({"password": FAKE_PW}),
    )
    .await
    .unwrap();
    assert_eq!(s, 401, "最初密码 401");

    // 文件 = 最终密码（中途值全被覆盖）
    let on_disk = std::fs::read_to_string(&path).unwrap();
    assert!(on_disk.contains("pw-test-matrix-3"), "{on_disk}");
    assert!(!on_disk.contains("pw-test-matrix-2"), "{on_disk}");
    cleanup(&path);
}

/* ══════════════════════════════════════════════════════════════════
   ⑦ oneshot 基座冒烟（listen 槽缺失路径：PUT listen → 503）
   ══════════════════════════════════════════════════════════════════ */

#[tokio::test]
async fn put_listen_without_supervisor_is_503() {
    let path = temp_config("nosup", &local_cfg());
    let state = build_state(Config::load(&path).unwrap(), path.clone());
    let app = build_app(state);
    let resp = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/v1/admin/listen")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"addr":"127.0.0.1:1"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 503, "无 supervisor → 503");
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert!(
        v["error"].as_str().unwrap().contains("listen supervisor"),
        "{v}"
    );
    cleanup(&path);
}
