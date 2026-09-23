//! Config 加载（M0.8 · P0-1 迁入 jev-switch-daemon · A4 增 `[[routes]]`）
//!
//! 路径：`JEV_SWITCH_CONFIG` 环境变量，或默认 `~/.jev-switch/providers.toml`。
//! 内容：provider 表 + 模型路由 DAG 边：
//! - `[[routes]]`（contracts/03 §2 正式形态：left/match/right/upstream_model/
//!   priority/sticky/on_error）
//! - 旧 `[router] "model" = "upstream"` 扁平表**兼容保留** —— 等价于单条
//!   `match=exact` 边、`priority=0`、默认策略（08 §3 明文）
//!
//! 相对路径基准：`JEV_SWITCH_CONFIG` 若是相对路径，按**启动 cargo/二进制时的
//! 进程 CWD**（工作区命令约定为仓库根）解析 —— 不是 manifest 目录、不是
//! 配置文件自身位置。拆 workspace 不改变此基准（CWD 由调用方决定）。
//!
//! 其余简化：
//! - 单文件（不分 server/observability/cli 等段）
//! - api_key 从 `api_key_env` 字段读环境变量
//! - **加载时检环**（contracts/03 §4 DAG 约束）：合并后的边图含环 → 拒绝加载

use jev_core::router::{check_acyclic, RouteEdge};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("config file not found: {0}")]
    NotFound(PathBuf),
    #[error("config read error: {0}")]
    Io(#[from] std::io::Error),
    #[error("config parse error: {0}")]
    Toml(#[from] toml::de::Error),
    #[error("missing environment variable '{var}' required by provider '{provider}'")]
    MissingEnv { provider: String, var: String },
    #[error("provider '{0}' missing required field 'kind'")]
    MissingKind(String),
    #[error("invalid route graph: {0}")]
    Cycle(String),
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct ProviderConfig {
    pub kind: String,
    pub base: String,
    #[serde(default)]
    pub api_key_env: Option<String>,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

fn default_enabled() -> bool {
    true
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub providers: HashMap<String, ProviderConfig>,
    /// 旧式扁平映射（兼容：等价单条 exact 边，priority=0）。
    #[serde(default)]
    pub router: HashMap<String, String>,
    /// 模型路由 DAG 边（contracts/03 §2 `[[routes]]`）。
    #[serde(default)]
    pub routes: Vec<RouteEdge>,
}

impl Config {
    /// 加载 config 文件。
    ///
    /// 路径优先：`JEV_SWITCH_CONFIG` 环境变量 → 否则 `~/.jev-switch/providers.toml`。
    /// 相对路径按进程 CWD 解析（见模块文档）。
    /// **加载即检环**：合并后的边图含环 → [`ConfigError::Cycle`]（DAG 约束）。
    pub fn load_default() -> Result<Self, ConfigError> {
        let path = std::env::var("JEV_SWITCH_CONFIG")
            .map(PathBuf::from)
            .unwrap_or_else(|_| default_config_path());
        Self::load(&path)
    }

    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        if !path.exists() {
            return Err(ConfigError::NotFound(path.to_path_buf()));
        }
        let raw = std::fs::read_to_string(path)?;
        let cfg: Config = toml::from_str(&raw)?;
        check_acyclic(&cfg.route_edges()).map_err(|e| ConfigError::Cycle(e.to_string()))?;
        Ok(cfg)
    }

    /// 合并全部路由边：旧 `[router]` 扁平表（→ 单 exact 边，priority=0）在前，
    /// `[[routes]]` 声明序在后（priority 排序发生在 `Router::select`，此处仅并集）。
    pub fn route_edges(&self) -> Vec<RouteEdge> {
        let mut edges: Vec<RouteEdge> = self
            .router
            .iter()
            .map(|(m, u)| RouteEdge::from_flat(m, u))
            .collect();
        edges.sort_by(|a, b| a.left.cmp(&b.left)); // 旧表序稳定（与 HashMap 迭代无关）
        edges.extend(self.routes.iter().cloned());
        edges
    }

    /// 校验合并后的边图无环（供不走 `load` 的构造路径调用）。
    pub fn validate_routes(&self) -> Result<(), ConfigError> {
        check_acyclic(&self.route_edges()).map_err(|e| ConfigError::Cycle(e.to_string()))
    }

    /// 读 provider 的 API key（按 `api_key_env` 字段读 env）。
    pub fn read_api_key(&self, provider_id: &str) -> Result<String, ConfigError> {
        let p = self
            .providers
            .get(provider_id)
            .ok_or_else(|| ConfigError::MissingKind(provider_id.into()))?;
        let var = p.api_key_env.as_deref().ok_or_else(|| ConfigError::MissingEnv {
            provider: provider_id.into(),
            var: "<api_key_env unset>".into(),
        })?;
        std::env::var(var).map_err(|_| ConfigError::MissingEnv {
            provider: provider_id.into(),
            var: var.into(),
        })
    }
}

fn default_config_path() -> PathBuf {
    // Windows: %USERPROFILE%\.jev-switch\providers.toml
    // Unix:    $HOME/.jev-switch/providers.toml
    if let Ok(home) = std::env::var("USERPROFILE") {
        PathBuf::from(home).join(".jev-switch").join("providers.toml")
    } else if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home).join(".jev-switch").join("providers.toml")
    } else {
        PathBuf::from("providers.toml")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_minimal_config() {
        let toml = r#"
[providers.vercel]
kind = "vercel"
base = "https://x"
api_key_env = "X"
enabled = true

[providers.laya]
kind = "laya"
base = "http://x"
enabled = true

[router]
"laya-english" = "laya"
"typesafe-ai/jev" = "vercel"
"#;
        let cfg: Config = toml::from_str(toml).unwrap();
        assert_eq!(cfg.providers.len(), 2);
        assert_eq!(cfg.router.get("laya-english"), Some(&"laya".into()));
        assert_eq!(
            cfg.router.get("typesafe-ai/jev"),
            Some(&"vercel".into())
        );
    }

    #[test]
    fn missing_file() {
        let r = Config::load(Path::new("nonexistent.toml"));
        assert!(matches!(r, Err(ConfigError::NotFound(_))));
    }

    /* ── A4：[[routes]] + 兼容 + 检环 ─────────────────────────── */

    #[test]
    fn parse_routes_with_defaults() {
        let toml = r#"
[[routes]]
left = "jev"
right = "vercel"
upstream_model = "typesafe-ai/jev"
priority = 10

[[routes]]
left = "jev"
match = "exact"
right = "laya"
priority = 30
sticky = "session"
on_error = "next"
"#;
        let cfg: Config = toml::from_str(toml).unwrap();
        assert_eq!(cfg.routes.len(), 2);
        let e0 = &cfg.routes[0];
        assert_eq!(e0.left, "jev");
        assert_eq!(e0.r#match, jev_core::router::MatchMode::Exact); // 默认 exact
        assert_eq!(e0.on_error, jev_core::router::OnError::Next); // 默认 next
        assert_eq!(e0.sticky, jev_core::router::Sticky::None); // 默认 none
        assert_eq!(e0.upstream_model.as_deref(), Some("typesafe-ai/jev"));
        assert_eq!(cfg.routes[1].sticky, jev_core::router::Sticky::Session);
    }

    #[test]
    fn old_router_table_merges_as_exact_edges() {
        // 验收 ⑤：旧 [router] 表行为回归不变（= 单条 exact 边）
        let toml = r#"
[router]
"laya-english" = "laya"
"typesafe-ai/jev" = "vercel"
"#;
        let cfg: Config = toml::from_str(toml).unwrap();
        let edges = cfg.route_edges();
        assert_eq!(edges.len(), 2);
        // 稳定排序后：laya-english 在 typesafe-ai/jev 前
        assert_eq!(edges[0].left, "laya-english");
        assert_eq!(edges[0].right, "laya");
        assert_eq!(edges[0].r#match, jev_core::router::MatchMode::Exact);
        assert_eq!(edges[0].priority, 0);
        assert_eq!(edges[1].left, "typesafe-ai/jev");
        assert_eq!(edges[1].right, "vercel");
        cfg.validate_routes().unwrap();
    }

    #[test]
    fn routes_and_router_merge_together() {
        let toml = r#"
[router]
"laya-english" = "laya"

[[routes]]
left = "jev"
right = "vercel"
priority = 10
"#;
        let cfg: Config = toml::from_str(toml).unwrap();
        let edges = cfg.route_edges();
        assert_eq!(edges.len(), 2);
        assert!(edges.iter().any(|e| e.left == "laya-english" && e.right == "laya"));
        assert!(edges.iter().any(|e| e.left == "jev" && e.right == "vercel"));
    }

    #[test]
    fn cyclic_routes_rejected_at_load() {
        // 验收 ④：含环配置加载被拒（经 load() 全路径）
        let dir = std::env::temp_dir().join(format!("jev-cfg-cycle-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("cycle.toml");
        std::fs::write(
            &path,
            r#"
[[routes]]
left = "a"
right = "b"

[[routes]]
left = "b"
right = "a"
"#,
        )
        .unwrap();
        let err = Config::load(&path).unwrap_err();
        assert!(matches!(err, ConfigError::Cycle(_)), "got: {err}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn acyclic_routes_load_ok() {
        let dir = std::env::temp_dir().join(format!("jev-cfg-dag-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("dag.toml");
        std::fs::write(
            &path,
            r#"
[[routes]]
left = "jev"
right = "jev-fast"
priority = 5

[[routes]]
left = "jev-fast"
right = "vercel"
priority = 10

[[routes]]
left = "jev"
right = "laya"
priority = 30

[[routes]]
left = "local/*"
match = "prefix"
right = "laya"
priority = 40
"#,
        )
        .unwrap();
        let cfg = Config::load(&path).unwrap();
        assert_eq!(cfg.routes.len(), 4);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_env_var() {
        let toml = r#"
[providers.foo]
kind = "vercel"
base = "https://x"
api_key_env = "JEV_TEST_NONEXISTENT_VAR_XYZ"
enabled = true
"#;
        let cfg: Config = toml::from_str(toml).unwrap();
        let err = cfg.read_api_key("foo").unwrap_err();
        match err {
            ConfigError::MissingEnv { provider, var } => {
                assert_eq!(provider, "foo");
                assert_eq!(var, "JEV_TEST_NONEXISTENT_VAR_XYZ");
            }
            _ => panic!("expected MissingEnv"),
        }
    }

    #[test]
    fn disabled_provider_keeps_in_config() {
        let toml = r#"
[providers.vercel]
kind = "vercel"
base = "https://x"
enabled = false
"#;
        let cfg: Config = toml::from_str(toml).unwrap();
        let p = cfg.providers.get("vercel").unwrap();
        assert!(!p.enabled);
    }
}
