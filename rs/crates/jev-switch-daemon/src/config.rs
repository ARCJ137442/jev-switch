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
//! - **密钥读取**（Q4=b / contracts/04 §1 默认档）：明文 `api_key` 字段**优先**；
//!   `api_key_env`（读环境变量）**兼容保留** —— 二者并存时明文赢
//! - **文件权限**：`providers.toml` 0600、`~/.jev-switch/` 0700（Unix 落地；
//!   Windows 按用户裁决降级为启动/写回 warning）
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
#[allow(dead_code)] // kind 目前只作配置标注（装配按 provider id 选 adapter），随 toml 往返保留
pub struct ProviderConfig {
    pub kind: String,
    pub base: String,
    /// 明文密钥（Q4=b 默认档；0600 文件）。**读取优先级高于 `api_key_env`**。
    /// 仅存在于 daemon —— 永不进 GET 响应 / 日志 / tracing（contracts/04 §2）。
    #[serde(default)]
    pub api_key: Option<String>,
    /// 环境变量名（兼容保留；明文缺省时回退到此）。
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
    /// 解析配置文件路径（`JEV_SWITCH_CONFIG` → 否则 `~/.jev-switch/providers.toml`）。
    /// admin 读改写同一文件时也用它（contracts/04 §1 真值源）。
    pub fn resolve_path() -> PathBuf {
        std::env::var("JEV_SWITCH_CONFIG")
            .map(PathBuf::from)
            .unwrap_or_else(|_| default_config_path())
    }

    /// 加载 config 文件。
    ///
    /// 路径优先：`JEV_SWITCH_CONFIG` 环境变量 → 否则 `~/.jev-switch/providers.toml`。
    /// 相对路径按进程 CWD 解析（见模块文档）。
    /// **加载即检环**：合并后的边图含环 → [`ConfigError::Cycle`]（DAG 约束）。
    pub fn load_default() -> Result<Self, ConfigError> {
        Self::load(&Self::resolve_path())
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

    /// 读 provider 的 API key。
    ///
    /// 优先级（Q4=b + 任务书「明文字段优先读，env 兼容保留」）：
    /// 1. 明文 `api_key`（非空）→ 直接返回，不查 env
    /// 2. 否则 `api_key_env` → 读环境变量
    /// 3. 两者皆无 / env 缺失 → [`ConfigError::MissingEnv`]
    pub fn read_api_key(&self, provider_id: &str) -> Result<String, ConfigError> {
        let p = self
            .providers
            .get(provider_id)
            .ok_or_else(|| ConfigError::MissingKind(provider_id.into()))?;
        if let Some(k) = p.api_key.as_deref() {
            if !k.is_empty() {
                return Ok(k.to_string());
            }
        }
        let var = p.api_key_env.as_deref().ok_or_else(|| ConfigError::MissingEnv {
            provider: provider_id.into(),
            var: "<api_key unset and api_key_env unset>".into(),
        })?;
        std::env::var(var).map_err(|_| ConfigError::MissingEnv {
            provider: provider_id.into(),
            var: var.into(),
        })
    }

    /// provider 的有效密钥（明文或 env 解析成功 → `Some`；否则 `None`）。
    /// admin GET 掩码 / `api_key_set` 用；**不回传明文**。
    pub fn effective_api_key(&self, provider_id: &str) -> Option<String> {
        self.read_api_key(provider_id).ok().filter(|k| !k.is_empty())
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

/* ══════════════════════════════════════════════════════════════════
   文件权限（contracts/04 §1：providers.toml 0600、目录 0700）
   Unix：真实 chmod；Windows：NTFS ACL 不在本期 —— 按用户裁决降级为 warning。
   ══════════════════════════════════════════════════════════════════ */

/// 写回配置文件后收紧为 **0600**（admin PUT 落盘路径调用）。
///
/// - Unix：`chmod 0600`；失败 → warning（不中断写入）
/// - Windows：无 POSIX 权限位 —— **降级为 warning**（用户已裁决的平台降级）
pub fn enforce_config_perms(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perm = std::fs::Permissions::from_mode(0o600);
        if let Err(e) = std::fs::set_permissions(path, perm) {
            tracing::warn!(path = %path.display(), error = %e,
                "failed to chmod 0600 providers.toml");
        }
    }
    #[cfg(windows)]
    {
        let _ = path;
        tracing::warn!(
            "providers.toml 0600 权限无法在 Windows 强制（POSIX 权限位不适用；NTFS ACL 不在本期）—— 按用户裁决降级为本警告。请确认配置目录仅本用户可读。"
        );
    }
}

/// 配置**目录**收紧为 **0700**（`~/.jev-switch/`；contracts/04 §1）。同平台策略。
pub fn enforce_dir_perms(dir: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perm = std::fs::Permissions::from_mode(0o700);
        if let Err(e) = std::fs::set_permissions(dir, perm) {
            tracing::warn!(dir = %dir.display(), error = %e,
                "failed to chmod 0700 config dir");
        }
    }
    #[cfg(windows)]
    {
        let _ = dir;
        tracing::warn!(
            "配置目录 0700 权限无法在 Windows 强制 —— 按用户裁决降级为本警告。"
        );
    }
}

/// 启动时检查配置文件权限并告警（contracts/04 §7 验收）。
///
/// - Unix：实际读 mode；非 0600 → warning（列出当前权限）
/// - Windows：无法判定 POSIX 位 → 恒 warning（平台降级，用户已裁决）
pub fn check_config_perms_warn(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        match std::fs::metadata(path) {
            Ok(md) => {
                let mode = md.permissions().mode() & 0o777;
                if mode != 0o600 {
                    tracing::warn!(path = %path.display(), mode = format_args!("{mode:o}"),
                        "providers.toml 权限为 {mode:o}，期望 0600（contracts/04 §1）—— 请收紧");
                }
            }
            Err(e) => tracing::warn!(path = %path.display(), error = %e,
                "无法读取 providers.toml 权限"),
        }
    }
    #[cfg(windows)]
    {
        let _ = path;
        tracing::warn!(
            "启动权限检查：Windows 无 POSIX 权限位，providers.toml 0600 检查降级为本警告（用户已裁决）—— 请确认配置文件仅本用户可读。"
        );
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

    /* ── A7：明文 api_key 优先 + env 兼容 ────────────────────────── */

    #[test]
    fn plaintext_api_key_read_and_takes_precedence_over_env() {
        // 明文优先（Q4=b 默认档）；env 指向不存在的变量 —— 明文赢则不报错
        let toml = r#"
[providers.vercel]
kind = "vercel"
base = "https://x"
api_key = "sk-test1234abcd"
api_key_env = "JEV_SWITCH_TEST_NO_SUCH_VAR_ZZZ"
enabled = true
"#;
        let cfg: Config = toml::from_str(toml).unwrap();
        assert_eq!(
            cfg.read_api_key("vercel").unwrap(),
            "sk-test1234abcd",
            "明文字段必须优先于 env"
        );
        assert_eq!(
            cfg.effective_api_key("vercel").as_deref(),
            Some("sk-test1234abcd")
        );
    }

    #[test]
    fn env_fallback_used_when_plaintext_absent() {
        // 明文缺省 → env 兼容路径保留（本测试 env 未设 → MissingEnv）
        let toml = r#"
[providers.vercel]
kind = "vercel"
base = "https://x"
api_key_env = "JEV_SWITCH_TEST_NO_SUCH_VAR_ZZZ"
enabled = true
"#;
        let cfg: Config = toml::from_str(toml).unwrap();
        assert!(cfg.providers["vercel"].api_key.is_none());
        let err = cfg.read_api_key("vercel").unwrap_err();
        assert!(matches!(err, ConfigError::MissingEnv { .. }), "got: {err}");
        assert_eq!(cfg.effective_api_key("vercel"), None);
    }

    #[test]
    fn empty_plaintext_falls_back_to_env() {
        // PUT 清除语义：明文空串视为未设 → 回退 env
        let toml = r#"
[providers.vercel]
kind = "vercel"
base = "https://x"
api_key = ""
api_key_env = "JEV_SWITCH_TEST_NO_SUCH_VAR_ZZZ"
enabled = true
"#;
        let cfg: Config = toml::from_str(toml).unwrap();
        assert!(matches!(
            cfg.read_api_key("vercel"),
            Err(ConfigError::MissingEnv { .. })
        ));
    }

    /* ── A7：文件权限 0600（Unix assert / Windows warning 降级） ── */

    fn touch_config(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "jev-perm-{}-{}",
            name,
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("providers.toml");
        std::fs::write(&path, "[providers.x]\nkind=\"k\"\nbase=\"b\"\n").unwrap();
        path
    }

    #[cfg(unix)]
    #[test]
    fn enforce_config_perms_sets_0600() {
        use std::os::unix::fs::PermissionsExt;
        let path = touch_config("unix");
        // 预置宽权限，验证 enforce 收紧
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        enforce_config_perms(&path);
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "写回后 providers.toml 必须 0600");
        let _ = std::fs::remove_file(&path);
    }

    #[cfg(unix)]
    #[test]
    fn enforce_dir_perms_sets_0700() {
        use std::os::unix::fs::PermissionsExt;
        let path = touch_config("unix-dir");
        let dir = path.parent().unwrap().to_path_buf();
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
        enforce_dir_perms(&dir);
        let mode = std::fs::metadata(&dir).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o700, "配置目录必须 0700");
        let _ = std::fs::remove_file(&path);
    }

    #[cfg(unix)]
    #[test]
    fn check_config_perms_warns_on_wrong_mode() {
        // 非 0600 → 不 panic，仅走告警路径（tracing 未初始化时静默）
        use std::os::unix::fs::PermissionsExt;
        let path = touch_config("unix-warn");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        check_config_perms_warn(&path); // 命中 warning 分支
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        check_config_perms_warn(&path); // 合规 → 无告警
        let _ = std::fs::remove_file(&path);
    }

    /// Windows 分支（用户已裁决降级）：无 POSIX 位可断言 ——
    /// 验证三函数均走 warning 路径且不报错（cfg 测）。
    #[cfg(windows)]
    #[test]
    fn windows_permission_helpers_degrade_to_warning() {
        let path = touch_config("win");
        enforce_config_perms(&path);
        enforce_dir_perms(path.parent().unwrap());
        check_config_perms_warn(&path);
        // 文件本体不受影响（warning 不是 error）
        assert!(path.exists());
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("[providers.x]"));
        let _ = std::fs::remove_file(&path);
    }
}
