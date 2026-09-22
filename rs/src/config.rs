//! Config 加载（M0.8）
//!
//! 路径：`JEV_SWITCH_CONFIG` 环境变量，或默认 `~/.jev-switch/providers.toml`。
//! 内容：2 个 provider + `[router] mapping` 段。
//!
//! MVP 简化：
//! - 单文件（不分 server/observability/cli 等段）
//! - api_key 从 `api_key_env` 字段读环境变量
//! - `[router]` 段是 `model_id = "upstream_id"` 的扁平表（toml 表里键是字符串，
//!   值是字符串 → HashMap<String, String>）

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
    #[serde(default)]
    pub router: HashMap<String, String>,
}

impl Config {
    /// 加载 config 文件。
    ///
    /// 路径优先：`JEV_SWITCH_CONFIG` 环境变量 → 否则 `~/.jev-switch/providers.toml`。
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
        Ok(cfg)
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
