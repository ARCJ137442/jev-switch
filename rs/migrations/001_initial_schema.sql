-- Jev-Switch 数据库初始化迁移脚本
-- 版本: 001
-- 描述: 创建初始 schema（5 个核心表 + 索引 + 初始数据）
-- 作者: Claude Opus 4.8
-- 日期: 2026-09-24

-- ============================================================================
-- 表 1: service_endpoints（服务入口配置）
-- ============================================================================

CREATE TABLE IF NOT EXISTS service_endpoints (
    id TEXT PRIMARY KEY,                    -- 入口 ID（对外暴露的模型 ID）
    strategy_config TEXT NOT NULL,          -- 策略配置（JSON 或 "follow_global"）
    enabled INTEGER NOT NULL DEFAULT 1,     -- 启用状态（0=禁用，1=启用）
    created_at INTEGER NOT NULL,            -- 创建时间（Unix timestamp）
    updated_at INTEGER NOT NULL             -- 更新时间（Unix timestamp）
);

CREATE INDEX IF NOT EXISTS idx_endpoints_enabled ON service_endpoints(enabled);

-- ============================================================================
-- 表 2: routes（路由配置）
-- ============================================================================

CREATE TABLE IF NOT EXISTS routes (
    id INTEGER PRIMARY KEY AUTOINCREMENT,   -- 自增 ID
    left_node TEXT NOT NULL,                -- 边起点（model ID / 别名 / 前缀）
    right_node TEXT NOT NULL,               -- 边终点（provider ID / 下游节点）
    upstream_model TEXT,                    -- 上游模型改写（可选）
    priority INTEGER NOT NULL,              -- 优先级（越小越优先）
    match_mode TEXT NOT NULL DEFAULT 'exact', -- 匹配模式（exact / prefix）
    sticky TEXT NOT NULL DEFAULT 'none',    -- 粘性策略（none / session）
    on_error TEXT NOT NULL DEFAULT 'next',  -- 失败策略（next / fail）
    created_at INTEGER NOT NULL,            -- 创建时间
    updated_at INTEGER NOT NULL             -- 更新时间
);

CREATE INDEX IF NOT EXISTS idx_routes_left ON routes(left_node);
CREATE INDEX IF NOT EXISTS idx_routes_right ON routes(right_node);
CREATE INDEX IF NOT EXISTS idx_routes_priority ON routes(left_node, priority);

-- ============================================================================
-- 表 3: call_logs（API 调用记录）
-- ============================================================================

CREATE TABLE IF NOT EXISTS call_logs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,   -- 自增 ID
    timestamp INTEGER NOT NULL,             -- 调用时间戳（Unix timestamp）
    endpoint_id TEXT NOT NULL,              -- 服务入口 ID
    route_key TEXT NOT NULL,                -- 路由 key（格式：left→right）
    upstream_provider TEXT NOT NULL,        -- 实际调用的 provider
    upstream_model TEXT NOT NULL,           -- 实际调用的模型
    success INTEGER NOT NULL,               -- 成功状态（0=失败，1=成功）
    latency_ms INTEGER NOT NULL,            -- 延迟（毫秒）
    error_message TEXT,                     -- 失败时的错误信息
    FOREIGN KEY (endpoint_id) REFERENCES service_endpoints(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_call_logs_timestamp ON call_logs(timestamp);
CREATE INDEX IF NOT EXISTS idx_call_logs_endpoint ON call_logs(endpoint_id, timestamp);
CREATE INDEX IF NOT EXISTS idx_call_logs_route ON call_logs(route_key, timestamp);
CREATE INDEX IF NOT EXISTS idx_call_logs_success ON call_logs(success, timestamp);

-- ============================================================================
-- 表 4: global_config（全局配置）
-- ============================================================================

CREATE TABLE IF NOT EXISTS global_config (
    key TEXT PRIMARY KEY,                   -- 配置键
    value TEXT NOT NULL,                    -- 配置值（JSON 或纯文本）
    updated_at INTEGER NOT NULL             -- 更新时间
);

-- 初始数据：默认策略 + 日志保留期限
INSERT OR IGNORE INTO global_config (key, value, updated_at) VALUES
    ('default_strategy', 'failover', strftime('%s', 'now')),
    ('log_retention_days', '30', strftime('%s', 'now'));

-- ============================================================================
-- 表 5: migrations（迁移版本标记）
-- ============================================================================

CREATE TABLE IF NOT EXISTS migrations (
    version INTEGER PRIMARY KEY,            -- 迁移版本号
    description TEXT NOT NULL,              -- 迁移描述
    applied_at INTEGER NOT NULL             -- 执行时间
);

-- 写入当前迁移标记
INSERT OR IGNORE INTO migrations (version, description, applied_at)
VALUES (1, 'initial schema', strftime('%s', 'now'));
