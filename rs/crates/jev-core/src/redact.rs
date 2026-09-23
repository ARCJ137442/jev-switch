//! 密钥脱敏（contracts/04 §2「Redact 约定」—— 放 core 供 ErrorBody / tracing /
//! admin 响应共用，零新依赖：手写扫描器，不引 regex crate）。
//!
//! 约定原文：
//! - 形态：`sk-****a1b2`（保留类型前缀 3–4 字符 + 末 4）
//! - 匹配：配置中所有 `api_key` 值；通用模式 `sk-[A-Za-z0-9_-]{8,}`、`Bearer\s+\S+`
//! - 范围：`ErrorBody`、`tracing`、admin audit、导出的 trace
//!
//! 执行顺序：已知明文 key（最长优先整串替换）→ 通用 `sk-` 模式 → `Bearer` 令牌。
//! 掩码结果对 `sk-` 前缀形态幂等（重复 redact 不再变形）。

/// 掩一枚 key：类型前缀 3–4 字符 + `****` + 末 4（contracts/04 §2 形态）。
///
/// - `sk-…` 类型前缀取 3 字符（`sk-`），否则取 4 字符
/// - 总长 < 8 无法在保留末 4 的同时藏住中间 → 只回 `****`
pub fn mask_key(key: &str) -> String {
    let chars: Vec<char> = key.chars().collect();
    if chars.len() < 8 {
        return "****".to_string();
    }
    let prefix_len = if key.starts_with("sk-") { 3 } else { 4 };
    let prefix: String = chars[..prefix_len].iter().collect();
    let suffix: String = chars[chars.len() - 4..].iter().collect();
    format!("{prefix}****{suffix}")
}

/// 脱敏一段任意文本：
/// 1. 命中的已知明文 key → [`mask_key`]（按长度降序替换，防前缀截胡）
/// 2. 通用 `sk-[A-Za-z0-9_-]{8,}` → [`mask_key`]
/// 3. `Bearer\s+\S+` → `Bearer <mask_key(token)>`（已含 `****` 的令牌不二次变形）
///
/// `known_keys` 来自配置里全部 `api_key` 值 + 启动时实际读到的 env key。
pub fn redact(input: &str, known_keys: &[String]) -> String {
    // 1. 已知明文（长的先换，避免短 key 是长 key 前缀时留尾巴）
    let mut keys: Vec<&String> = known_keys.iter().filter(|k| !k.is_empty()).collect();
    keys.sort_by_key(|k| std::cmp::Reverse(k.chars().count()));
    let mut out = input.to_string();
    for k in keys {
        if out.contains(k.as_str()) {
            out = out.replace(k.as_str(), &mask_key(k));
        }
    }
    // 2. 通用 sk- 模式
    out = redact_sk_pattern(&out);
    // 3. Bearer 令牌
    redact_bearer(&out)
}

/// 通用 `sk-[A-Za-z0-9_-]{8,}`：`sk-` 后至少 8 个合法字符才算一枚 token。
fn redact_sk_pattern(s: &str) -> String {
    let b = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(b.len());
    let mut i = 0usize;
    while i < b.len() {
        if b[i..].starts_with(b"sk-") {
            let mut j = i + 3;
            while j < b.len() && (b[j].is_ascii_alphanumeric() || b[j] == b'_' || b[j] == b'-') {
                j += 1;
            }
            if j - (i + 3) >= 8 {
                // token = s[i..j]（纯 ASCII 字节，边界安全）
                out.extend_from_slice(mask_key(&s[i..j]).as_bytes());
                i = j;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    // 只在 ASCII 匹配点切割、原样搬运其余字节 → UTF-8 序列完整
    String::from_utf8(out).unwrap_or_else(|_| s.to_string())
}

/// `Bearer\s+\S+` → `Bearer<原空白><mask(token)>`；已掩码（含 `****`）的令牌跳过。
fn redact_bearer(s: &str) -> String {
    let b = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(b.len());
    let mut i = 0usize;
    while i < b.len() {
        if b[i..].starts_with(b"Bearer") {
            let after_kw = i + 6;
            // \s+ 至少一个空白
            let mut ws_end = after_kw;
            while ws_end < b.len() && b[ws_end].is_ascii_whitespace() {
                ws_end += 1;
            }
            if ws_end > after_kw {
                let mut tok_end = ws_end;
                while tok_end < b.len() && !b[tok_end].is_ascii_whitespace() {
                    tok_end += 1;
                }
                if tok_end > ws_end {
                    let token = &s[ws_end..tok_end];
                    out.extend_from_slice(b"Bearer");
                    out.extend_from_slice(&b[after_kw..ws_end]);
                    if token.contains("****") {
                        out.extend_from_slice(token.as_bytes()); // 已掩码：不二次变形
                    } else {
                        out.extend_from_slice(mask_key(token).as_bytes());
                    }
                    i = tok_end;
                    continue;
                }
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8(out).unwrap_or_else(|_| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(ks: &[&str]) -> Vec<String> {
        ks.iter().map(|s| s.to_string()).collect()
    }

    /* ── mask_key 形态（contracts/04 §2：sk-****a1b2） ─────────── */

    #[test]
    fn mask_key_keeps_sk_prefix_and_last4() {
        assert_eq!(mask_key("sk-test1234abcd"), "sk-****abcd");
        // 非 sk- 前缀：类型前缀取 4 字符
        assert_eq!(mask_key("BearerLongSecretwxyz"), "Bear****wxyz");
        // 过短：只回 ****
        assert_eq!(mask_key("sk-123"), "****");
        assert_eq!(mask_key(""), "****");
    }

    #[test]
    fn mask_key_idempotent_for_sk_shape() {
        let once = mask_key("sk-test1234abcd");
        assert_eq!(mask_key(&once), once, "sk- 形态重复掩码不变形");
    }

    /* ── redact 主路径（验收 ②） ──────────────────────────────── */

    #[test]
    fn redact_known_key_in_error_body() {
        // 单测②：错误体含 sk-… → 输出 masked
        let known = keys(&["sk-test1234abcd"]);
        let body = r#"{"error":"upstream vercel returned 401: invalid key sk-test1234abcd"}"#;
        let out = redact(body, &known);
        assert!(!out.contains("sk-test1234abcd"), "明文不得残留: {out}");
        assert!(out.contains("sk-****abcd"), "应为掩码形态: {out}");
        // 结构性 JSON 字段不受影响
        assert!(out.contains(r#"{"error""#));
    }

    #[test]
    fn redact_generic_sk_pattern_without_known_list() {
        // 通用模式：配置外的 sk-token 也拦（上游 4xx body 回显场景）
        let out = redact("bad key sk-abcdefghijklmnop leaked", &[]);
        assert!(!out.contains("sk-abcdefghijklmnop"), "{out}");
        assert!(out.contains("sk-****mnop"), "{out}");
    }

    #[test]
    fn redact_bearer_token_replaced() {
        // 单测②：Bearer 替换
        let out = redact("Authorization: Bearer secretToken987654", &[]);
        assert!(!out.contains("secretToken987654"), "{out}");
        assert!(out.starts_with("Authorization: Bearer "), "{out}");
        // 非 sk- token：前缀 4 + **** + 末 4（"secretToken987654" 末 4 = 7654）
        let token_masked = out.rsplit(' ').next().unwrap();
        assert_eq!(token_masked, "secr****7654");
    }

    #[test]
    fn redact_bearer_with_known_sk_key_inside() {
        let known = keys(&["sk-test1234abcd"]);
        let out = redact("Authorization: Bearer sk-test1234abcd", &known);
        assert!(!out.contains("sk-test1234abcd"), "{out}");
        // 先被通用/已知规则掩成 sk-****abcd，Bearer 步不二次变形
        assert!(out.contains("sk-****abcd"), "{out}");
    }

    #[test]
    fn redact_utf8_text_survives() {
        // 中英混排 + 非 ASCII 不被字节扫描器打碎
        let known = keys(&["sk-test1234abcd"]);
        let out = redact("错误：密钥 sk-test1234abcd 已泄露（test）", &known);
        assert!(out.contains("错误：密钥"), "UTF-8 被破坏: {out}");
        assert!(out.contains("sk-****abcd"), "{out}");
        assert!(!out.contains("sk-test1234abcd"), "{out}");
    }

    #[test]
    fn redact_no_key_unchanged() {
        let s = "all good, nothing secret here";
        assert_eq!(redact(s, &[]), s);
    }

    #[test]
    fn redact_sk_needs_at_least_8_after_prefix() {
        // sk- + 7 字符 = 不足 {8,} → 不动（契约模式字面）
        let out = redact("sk-abcdefg short", &[]);
        assert_eq!(out, "sk-abcdefg short");
        // 恰 8 → 掩
        let out8 = redact("sk-abcdefgh", &[]);
        assert_eq!(out8, "sk-****efgh");
    }
}
