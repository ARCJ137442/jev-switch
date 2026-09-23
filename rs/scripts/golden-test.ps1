# golden-test.ps1 — 黄金测试一键重建 + 跑通（contracts/02 §4）
#
# 在 **jev-switch 仓库之外** 脚手架一个 my_lab_adapter 外部 crate：
#   1. 自定义怪异 DTO {"verdict":"yes","odds":0.8}
#   2. impl ProtocolAdapter + impl UpstreamAdapter（零改 jev-core/jev-protocol/daemon）
#   3. registry.register(...) 后 POST /v1/systemone 换 model 命中
#   4. 响应已是标准 JevResponse（Noul { noul|probability }，无 confidence）
#
# 路径策略（无机器敏感信息）：
#   - 仓库根从脚本自身位置相对推导（$PSScriptRoot\..\..）
#   - 仓外落点 = $env:JEV_GOLDEN_DIR，默认系统 TEMP\jev-golden
#     （本机验收可用 JEV_GOLDEN_DIR=E:\tmp\jev-golden 覆盖到任务书指定位置）
#
# 用法：
#   pwsh -File rs/scripts/golden-test.ps1          # 或 powershell -File …
#   $env:JEV_GOLDEN_DIR='E:\tmp\jev-golden'; powershell -File rs/scripts/golden-test.ps1

$ErrorActionPreference = 'Stop'

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$protoPath = ((Join-Path $repoRoot 'rs\crates\jev-protocol')) -replace '\\', '/'
$corePath = ((Join-Path $repoRoot 'rs\crates\jev-core')) -replace '\\', '/'

$goldenRoot = $env:JEV_GOLDEN_DIR
if (-not $goldenRoot) {
    $goldenRoot = Join-Path ([System.IO.Path]::GetTempPath()) 'jev-golden'
}
$crate = Join-Path $goldenRoot 'my_lab_adapter'

Write-Host "== Jev-Switch golden test (contracts/02 §4) =="
Write-Host "repo:   $repoRoot"
Write-Host "crate:  $crate"

# 一键重建：整目录抹掉重写（保证从零可复制）
if (Test-Path $crate) { Remove-Item -Recurse -Force $crate }
New-Item -ItemType Directory -Force -Path (Join-Path $crate 'src') | Out-Null

$cargoToml = @'
# 外部 crate：只依赖 jev-protocol + jev-core（+ axum/tokio 自建最小服务）
# —— 黄金测试红线（contracts/02 §4）：加提供商不得改 core match / invoke 传参 /
# 往 JevResponse 塞厂商键。本 crate 全程零修改内核。
[package]
name = "my_lab_adapter"
version = "0.0.1"
edition = "2021"
publish = false

[dependencies]
jev-protocol = { path = "__PROTO__" }
jev-core = { path = "__CORE__" }
axum = "0.7"
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
async-trait = "0.1"
reqwest = { version = "0.12", features = ["json"] }
'@ -replace '__PROTO__', $protoPath -replace '__CORE__', $corePath
Set-Content -Path (Join-Path $crate 'Cargo.toml') -Value $cargoToml -Encoding utf8

$libRs = @'
//! my_lab_adapter —— Jev-Switch 黄金测试外部 crate（contracts/02 §4）。
//!
//! 验收点：
//! - 怪异厂商 DTO：`{"verdict":"yes","odds":0.8}`（只活在本 crate —— 厂商 DTO
//!   不进 jev-protocol；内核零改）
//! - `impl ProtocolAdapter`（incoming 解怪 DTO → 标准 JevResponse；outgoing 默认恒等）
//! - `impl UpstreamAdapter`（capabilities 自报；evaluate 走本 crate 方言）
//! - `Registry::register(Box<dyn UpstreamAdapter>)` + `invoke(req, ctx)` —— 签名零变
//! - 自建最小 axum 服务：`POST /v1/systemone` 换 model 命中（daemon 不可复用：
//!   daemon 是编译死的 bin —— 外部 crate 自组 server + Registry 即满足验收本意）

use axum::{
    body::Bytes,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::post,
    Json, Router,
};
use jev_core::adapter::{plain_ctx, IncomingCtx, ProtocolAdapter, Registry, UpstreamAdapter};
use jev_core::router::{MatchMode, RouteEdge};
use jev_core::upstream::{Capabilities, JevError, QuestionType};
use jev_protocol::{Answer, JevRequest, JevResponse, NoulAnswer, NoulKind};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

/* ── 1. 怪异厂商 DTO（只活在本 crate） ─────────────────────── */

/// 我 Lab 的怪响应：`{"verdict":"yes","odds":0.8}`。
/// `odds` = P(true)；`verdict` 是厂商裁决字面量。
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MyLabResponse {
    pub verdict: String,
    pub odds: f64,
}

/* ── 2a. ProtocolAdapter（冻结 trait） ─────────────────────── */

pub struct MyLabProtocol;

impl ProtocolAdapter for MyLabProtocol {
    // outgoing：默认恒等（本方言无需出站改写）

    fn incoming(&self, raw: &[u8], _ctx: &IncomingCtx<'_>) -> Result<JevResponse, JevError> {
        let parsed: MyLabResponse =
            serde_json::from_slice(raw).map_err(|e| JevError::BadResponse {
                upstream_id: "lab".into(),
                message: format!("parse my-lab DTO: {e}; raw={}", String::from_utf8_lossy(raw)),
            })?;
        let mut answers = BTreeMap::new();
        answers.insert(
            "q".to_string(),
            Answer::Noul(NoulAnswer {
                kind: NoulKind::Noul,
                noul: Some(parsed.odds),
                probability: None,
            }),
        );
        Ok(JevResponse {
            model: None,
            answers,
            usage: None,
            upstream_calls: Some(1),
            latency_ms: None,
            cost_usd: None,
            extra: BTreeMap::from([(
                "labVerdict".to_string(),
                serde_json::Value::String(parsed.verdict),
            )]),
        })
    }

    fn dialect(&self) -> &'static str {
        "my_lab_v1"
    }
}

/* ── 2b. UpstreamAdapter（冻结 trait） ─────────────────────── */

pub const LAB_CAPABILITIES: Capabilities = Capabilities {
    question_types: &[QuestionType::Noul],
    has_confidence: false,
    has_usage: false,
    noul_via_boolean: false,
    retryable_status: &[408, 429, 500, 502, 503, 504],
};

pub struct MyLabAdapter {
    id: String,
    protocol: MyLabProtocol,
    pub calls: AtomicU32,
}

impl MyLabAdapter {
    pub fn new() -> Self {
        Self {
            id: "lab".into(),
            protocol: MyLabProtocol,
            calls: AtomicU32::new(0),
        }
    }
}

impl Default for MyLabAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl UpstreamAdapter for MyLabAdapter {
    fn id(&self) -> &str {
        &self.id
    }

    fn capabilities(&self) -> Capabilities {
        LAB_CAPABILITIES
    }

    async fn evaluate(&self, req: JevRequest) -> Result<JevResponse, JevError> {
        // 真实实现这里是厂商 HTTP 调用；黄金测试合成怪异 DTO bytes。
        self.calls.fetch_add(1, Ordering::SeqCst);
        let raw = br#"{"verdict":"yes","odds":0.8}"#;
        let ctx = IncomingCtx {
            request: &req,
            original: &req,
            status: 200,
        };
        let mut resp = self.protocol.incoming(raw, &ctx)?;
        resp.model = Some(req.model.clone());
        resp.upstream_calls = Some(self.calls.load(Ordering::SeqCst));
        Ok(resp)
    }
}

/* ── 3. 注册 + 最小 axum 服务 ─────────────────────────────── */

/// 路由边：对外 model id → 注册的上游 id（右列 = adapter id "lab"）。
pub fn lab_edges() -> Vec<RouteEdge> {
    let mk = |left: &str| RouteEdge {
        left: left.to_string(),
        r#match: MatchMode::Exact,
        right: "lab".to_string(),
        upstream_model: Some("lab-odds-v1".to_string()),
        priority: 0,
        sticky: Default::default(),
        on_error: Default::default(),
    };
    vec![mk("lab-judge"), mk("verdict-model")]
}

/// 零改内核的注册路径（签名 = 冻结的 `register(Box<dyn UpstreamAdapter>)`）。
pub fn build_registry() -> Registry {
    let mut registry = Registry::new(lab_edges());
    registry.register(Box::new(MyLabAdapter::new()));
    registry
}

#[derive(Clone)]
pub struct AppState {
    pub registry: Arc<Registry>,
}

#[derive(Debug, Serialize)]
pub struct ErrorBody {
    pub error: String,
    pub upstream: Option<String>,
    pub retryable: bool,
}

async fn systemone_handler(
    State(state): State<AppState>,
    body: Bytes,
) -> Result<Json<JevResponse>, Response> {
    let req: JevRequest = serde_json::from_slice(&body).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(ErrorBody {
                error: e.to_string(),
                upstream: None,
                retryable: false,
            }),
        )
            .into_response()
    })?;

    // invoke 签名零变（冻结）；sticky：无入口 → plain_ctx()
    match state.registry.invoke(req, plain_ctx()).await {
        Ok(resp) => Ok(Json(resp)),
        Err(e) => {
            let status =
                StatusCode::from_u16(e.http_status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            let body = ErrorBody {
                error: e.to_string(),
                upstream: e.error_body_upstream().map(str::to_string),
                retryable: e.retryable(),
            };
            Err((status, Json(body)).into_response())
        }
    }
}

/// 自建最小 `POST /v1/systemone` 服务（daemon 是编译死的 bin，不可复用 ——
/// 外部 crate 自组 server + Registry 即满足「注册后换 model 命中」验收本意）。
pub fn app(registry: Registry) -> Router {
    Router::new()
        .route("/v1/systemone", post(systemone_handler))
        .with_state(AppState {
            registry: Arc::new(registry),
        })
}
'@
Set-Content -Path (Join-Path $crate 'src\lib.rs') -Value $libRs -Encoding utf8

$testRs = @'
//! 黄金测试（contracts/02 §4 验收正文）：
//! 外部 crate 零改内核 → register → POST /v1/systemone 换 model 命中 →
//! 响应已是标准 JevResponse。

use my_lab_adapter::{app, build_registry, MyLabAdapter};
use jev_core::adapter::plain_ctx;
use jev_protocol::{Answer, JevRequest, JevResponse};
use std::collections::BTreeMap;

fn noul_request(model: &str) -> JevRequest {
    let mut q = BTreeMap::new();
    q.insert(
        "q".to_string(),
        jev_protocol::Question::Noul {
            instructions: "is it true?".into(),
            criteria: jev_protocol::Criteria::Bool {
                r#true: "yes".into(),
                r#false: "no".into(),
            },
        },
    );
    JevRequest {
        model: model.to_string(),
        state: serde_json::json!({"task": "golden"}),
        questions: q,
    }
}

/// 红线自查：标准响应形状 —— Noul { noul|probability }、无 confidence、type=noul。
fn assert_standard_noul(resp: &JevResponse, expect_noul: f64) {
    let wire = serde_json::to_value(&resp.answers["q"]).expect("answer serializes");
    assert_eq!(wire["type"], "noul", "语义层判别值必须是 noul: {wire}");
    assert!(wire.get("confidence").is_none(), "布尔族无 confidence: {wire}");
    let Answer::Noul(n) = &resp.answers["q"] else {
        panic!("expected Answer::Noul, got {:?}", resp.answers["q"]);
    };
    let got = jev_protocol::noul_probability(n);
    assert!((got - expect_noul).abs() < 1e-12, "noul={got}, want {expect_noul}");
}

#[tokio::test]
async fn invoke_zero_kernel_modification_yields_standard_jev_response() {
    let registry = build_registry();
    let resp = registry
        .invoke(noul_request("lab-judge"), plain_ctx())
        .await
        .expect("invoke 冻结签名直接命中");
    assert_standard_noul(&resp, 0.8);
    // 厂商键只进 extra（红线：不塞正式字段）
    assert_eq!(resp.extra.get("labVerdict"), Some(&serde_json::json!("yes")));
    // adapter 回显收到的 model = 已按边改写的 upstream_model（改写在 invoke→
    // evaluate 之间发生，evaluate 看到 lab-odds-v1 —— 证明边配置生效）
    assert_eq!(resp.model.as_deref(), Some("lab-odds-v1"));
}

#[tokio::test]
async fn post_systemone_switch_model_hits_and_unknown_404() {
    // 自建 axum 服务跑在随机端口（不占 11435）
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("addr");
    tokio::spawn(async move {
        axum::serve(listener, app(build_registry())).await.expect("serve");
    });
    let client = reqwest::Client::new();
    let url = format!("http://{addr}/v1/systemone");

    // 1) model "lab-judge" 命中
    let r = client
        .post(&url)
        .json(&noul_request("lab-judge"))
        .send()
        .await
        .expect("POST");
    assert_eq!(r.status().as_u16(), 200, "lab-judge 应命中");
    let resp: JevResponse = r.json().await.expect("JevResponse 形状");
    assert_standard_noul(&resp, 0.8);

    // 2) **换 model**："verdict-model" 同样命中同一注册上游（配置两条边）
    let r2 = client
        .post(&url)
        .json(&noul_request("verdict-model"))
        .send()
        .await
        .expect("POST");
    assert_eq!(r2.status().as_u16(), 200, "换 model 后仍应命中");
    let resp2: JevResponse = r2.json().await.expect("JevResponse 形状");
    assert_standard_noul(&resp2, 0.8);

    // 3) 未配置 model → 404
    let r3 = client
        .post(&url)
        .json(&noul_request("nope"))
        .send()
        .await
        .expect("POST");
    assert_eq!(r3.status().as_u16(), 404, "未知 model 应 404");

    // 4) adapter 计数：2 次命中 = 2 实发（invoke 覆写 upstream_calls）
    assert_eq!(resp.upstream_calls, Some(1));
    assert_eq!(resp2.upstream_calls, Some(1));
}

#[tokio::test]
async fn adapter_capability_self_reported_no_kernel_match() {
    // 能力注册制：外部 adapter 自报 —— 内核没有按 id 的 match 表可查
    let a = MyLabAdapter::new();
    let cap = jev_core::adapter::UpstreamAdapter::capabilities(&a);
    assert!(cap.supports(jev_protocol::QuestionType::Noul));
    assert!(!cap.supports(jev_protocol::QuestionType::Choice));
}
'@
# 集成测试放 crate 根 tests/（src/tests 不会被 cargo 当 integration test 收集）
New-Item -ItemType Directory -Force -Path (Join-Path $crate 'tests') | Out-Null
Set-Content -Path (Join-Path $crate 'tests\golden.rs') -Value $testRs -Encoding utf8

Write-Host "-- cargo test (external crate) --"
cargo test --manifest-path (Join-Path $crate 'Cargo.toml')
if ($LASTEXITCODE -ne 0) {
    Write-Error "GOLDEN TEST FAILED (exit $LASTEXITCODE)"
    exit $LASTEXITCODE
}
Write-Host "== GOLDEN TEST PASSED == $crate"
