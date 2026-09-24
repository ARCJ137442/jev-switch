# 数据库迁移完整方案

**日期**: 2026-09-24  
**版本**: 1.0  
**状态**: 设计定稿，待实施  
**作者**: Claude Opus 4.8

---

## §0 执行摘要

**一句话**: 将 Jev-Switch 从 TOML 配置文件迁移到 SQLite 数据库，支持服务入口配置、路由管理、调用统计，实现运行时热更新，零重启。

**核心目标**:
1. 从 `providers.toml` 迁移到 SQLite 数据库（5 个核心表）
2. daemon 启动时自动检测并执行迁移（向后兼容）
3. 支持服务入口配置（failover/race/负载均衡/影子模式）
4. 存储 API 调用历史（支持调用次数统计 + 成败分析）
5. 保留 TOML 文件作为只读备份（迁移后不再写入）

---

## §1 数据库设计

### 1.1 Schema 概览

| 表名 | 用途 | 记录数量级 |
|------|------|----------|
| `service_endpoints` | 服务入口配置 | ~10-50 |
| `routes` | 路由配置 | ~50-200 |
| `call_logs` | API 调用记录 | ~100K/月 |
| `global_config` | 全局配置 | ~5-10 |
| `migrations` | 迁移版本标记 | ~10 |

### 1.2 表 1: `service_endpoints`（服务入口）

**用途**: 存储对外暴露的模型 ID + 路由策略配置

```sql
CREATE TABLE service_endpoints (
    id TEXT PRIMARY KEY,                    -- 入口 ID（对外暴露的模型 ID）
    strategy_config TEXT NOT NULL,          -- 策略配置（JSON 或 "follow_global"）
    enabled INTEGER NOT NULL DEFAULT 1,     -- 启用状态（0=禁用，1=启用）
    created_at INTEGER NOT NULL,            -- 创建时间（Unix timestamp）
    updated_at INTEGER NOT NULL             -- 更新时间（Unix timestamp）
);
```

**`strategy_config` 格式**:
```json
// 跟随全局
"follow_global"

// 特定策略（failover）
{"type": "failover"}

// 特定策略（race）
{"type": "race", "params": {"timeout_ms": 5000}}

// 特定策略（load_balance）
{"type": "load_balance", "params": {"weight_mode": "priority"}}

// 特定策略（shadow）
{"type": "shadow", "params": {"shadow_target": "laya"}}
```

**索引**:
```sql
CREATE INDEX idx_endpoints_enabled ON service_endpoints(enabled);
```

### 1.3 表 2: `routes`（路由配置）

**用途**: 存储模型路由 DAG 边（从 TOML `[[routes]]` 迁移）

```sql
CREATE TABLE routes (
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
```

**字段说明**:
- `left_node`: 对应 TOML 的 `left` 字段
- `right_node`: 对应 TOML 的 `right` 字段
- `match_mode`: `"exact"` 或 `"prefix"`
- `sticky`: `"none"` 或 `"session"`
- `on_error`: `"next"` 或 `"fail"`

**索引**:
```sql
CREATE INDEX idx_routes_left ON routes(left_node);
CREATE INDEX idx_routes_right ON routes(right_node);
CREATE INDEX idx_routes_priority ON routes(left_node, priority);
```

### 1.4 表 3: `call_logs`（调用记录）

**用途**: 存储 API 调用历史，支持调用次数统计 + 成败分析

```sql
CREATE TABLE call_logs (
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
```

**索引**:
```sql
CREATE INDEX idx_call_logs_timestamp ON call_logs(timestamp);
CREATE INDEX idx_call_logs_endpoint ON call_logs(endpoint_id, timestamp);
CREATE INDEX idx_call_logs_route ON call_logs(route_key, timestamp);
CREATE INDEX idx_call_logs_success ON call_logs(success, timestamp);
```

**数据保留策略**: 默认保留最近 30 天的调用记录（可配置）

### 1.5 表 4: `global_config`（全局配置）

**用途**: 存储全局配置项（默认策略、数据保留期限等）

```sql
CREATE TABLE global_config (
    key TEXT PRIMARY KEY,                   -- 配置键
    value TEXT NOT NULL,                    -- 配置值（JSON 或纯文本）
    updated_at INTEGER NOT NULL             -- 更新时间
);
```

**初始数据**:
```sql
INSERT INTO global_config (key, value, updated_at) VALUES
    ('default_strategy', 'failover', strftime('%s', 'now')),
    ('log_retention_days', '30', strftime('%s', 'now'));
```

### 1.6 表 5: `migrations`（迁移标记）

**用途**: 记录已执行的迁移版本（防止重复迁移）

```sql
CREATE TABLE migrations (
    version INTEGER PRIMARY KEY,            -- 迁移版本号
    description TEXT NOT NULL,              -- 迁移描述
    applied_at INTEGER NOT NULL             -- 执行时间
);
```

---

## §2 迁移流程设计

### 2.1 迁移触发时机

**daemon 启动时自动检测**:
1. 检查 `~/.jev-switch/jev-switch.db` 是否存在
2. 若不存在 → 创建数据库 + 执行初始化迁移
3. 若存在 → 检查 `migrations` 表，执行未应用的迁移

### 2.2 迁移步骤详解

#### Step 1: 创建数据库文件

```rust
let db_path = config_dir.join("jev-switch.db");
let conn = rusqlite::Connection::open(&db_path)?;
```

#### Step 2: 执行 SQL 迁移脚本

```rust
// 读取 rs/migrations/001_initial_schema.sql
let sql = include_str!("../migrations/001_initial_schema.sql");
conn.execute_batch(sql)?;
```

#### Step 3: 从 TOML 迁移数据

**3.1 推导服务入口**:
- 从 `[[routes]]` 提取所有 `left` 字段去重
- 从旧 `[router]` 表提取所有 model ID
- 为每个 model ID 创建一个服务入口（默认策略：`follow_global`）

**3.2 迁移路由**:
- 从 `[[routes]]` 直接迁移（字段一一对应）
- 从旧 `[router]` 表转换为 `routes` 表（单条 exact 边）

**3.3 写入迁移标记**:
```sql
INSERT INTO migrations (version, description, applied_at)
VALUES (1, 'initial migration from TOML', strftime('%s', 'now'));
```

### 2.3 数据映射逻辑

#### 从 TOML `[[routes]]` → `routes` 表

| TOML 字段 | SQLite 字段 | 转换规则 |
|----------|------------|---------|
| `left` | `left_node` | 直接复制 |
| `right` | `right_node` | 直接复制 |
| `upstream_model` | `upstream_model` | 直接复制（可选） |
| `priority` | `priority` | 直接复制 |
| `match` | `match_mode` | 直接复制（默认 `"exact"`） |
| `sticky` | `sticky` | 直接复制（默认 `"none"`） |
| `on_error` | `on_error` | 直接复制（默认 `"next"`） |
| - | `created_at` | 当前时间戳 |
| - | `updated_at` | 当前时间戳 |

#### 从旧 `[router]` → `routes` 表

| TOML | SQLite |
|------|--------|
| `"model" = "upstream"` | `INSERT INTO routes (left_node, right_node, priority, match_mode, sticky, on_error, created_at, updated_at) VALUES ('model', 'upstream', 0, 'exact', 'none', 'next', NOW, NOW)` |

#### 推导服务入口

```rust
// 伪代码
let mut model_ids = HashSet::new();

// 从 [[routes]] 提取
for route in config.routes {
    model_ids.insert(route.left.clone());
}

// 从 [router] 提取
for (model, _) in config.router {
    model_ids.insert(model.clone());
}

// 为每个 model_id 创建服务入口
for model_id in model_ids {
    db.execute(
        "INSERT INTO service_endpoints (id, strategy_config, enabled, created_at, updated_at)
         VALUES (?, 'follow_global', 1, ?, ?)",
        (model_id, now, now)
    )?;
}
```

---

## §3 daemon 加载逻辑

### 3.1 启动时加载流程

```rust
// 伪代码
async fn load_config() -> Result<AppConfig> {
    let db_path = get_db_path();
    
    // 1. 检查数据库是否存在
    if !db_path.exists() {
        // 首次启动：执行迁移
        migrate_from_toml(&db_path)?;
    }
    
    // 2. 从数据库加载配置
    let conn = Connection::open(&db_path)?;
    
    let endpoints = load_service_endpoints(&conn)?;
    let routes = load_routes(&conn)?;
    let global_config = load_global_config(&conn)?;
    
    Ok(AppConfig {
        endpoints,
        routes,
        global_config,
    })
}
```

### 3.2 内存数据结构

```rust
pub struct AppConfig {
    pub endpoints: HashMap<String, ServiceEndpoint>,
    pub routes: Vec<RouteEdge>,
    pub global_config: GlobalConfig,
}

pub struct ServiceEndpoint {
    pub id: String,
    pub strategy_config: StrategyConfig,
    pub enabled: bool,
}

pub enum StrategyConfig {
    FollowGlobal,
    Specific(Strategy),
}

pub enum Strategy {
    Failover,
    Race { timeout_ms: u64 },
    LoadBalance { weight_mode: String },
    Shadow { shadow_target: String },
}
```

### 3.3 运行时热更新

**写入数据库后立即生效**（无需重启）:
```rust
// 示例：更新服务入口策略
pub fn update_endpoint_strategy(
    &self,
    endpoint_id: &str,
    strategy: StrategyConfig,
) -> Result<()> {
    // 1. 写入数据库
    self.db.execute(
        "UPDATE service_endpoints SET strategy_config = ?, updated_at = ?
         WHERE id = ?",
        (serde_json::to_string(&strategy)?, now(), endpoint_id)
    )?;
    
    // 2. 更新内存（RwLock）
    let mut endpoints = self.endpoints.write().unwrap();
    if let Some(ep) = endpoints.get_mut(endpoint_id) {
        ep.strategy_config = strategy;
    }
    
    Ok(())
}
```

---

## §4 数据保留策略

### 4.1 `call_logs` 自动清理机制

**触发时机**: 每天凌晨 3 点（或每次 daemon 启动时检查）

**清理逻辑**:
```sql
-- 删除超过保留期限的记录
DELETE FROM call_logs
WHERE timestamp < strftime('%s', 'now') - (
    SELECT CAST(value AS INTEGER) * 86400
    FROM global_config
    WHERE key = 'log_retention_days'
);
```

**可配置参数**:
- `log_retention_days`: 保留天数（默认 30 天）
- 通过 `PUT /v1/admin/config/log_retention_days` 修改

### 4.2 清理任务实现

```rust
// 伪代码
pub async fn start_cleanup_task(db: Arc<Database>) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(86400)); // 24 小时
        loop {
            interval.tick().await;
            if let Err(e) = cleanup_old_logs(&db).await {
                tracing::error!("cleanup failed: {}", e);
            }
        }
    });
}

async fn cleanup_old_logs(db: &Database) -> Result<()> {
    let deleted = db.execute(
        "DELETE FROM call_logs WHERE timestamp < strftime('%s', 'now') - 
         (SELECT CAST(value AS INTEGER) * 86400 FROM global_config WHERE key = 'log_retention_days')",
        []
    )?;
    tracing::info!("cleaned up {} old call logs", deleted);
    Ok(())
}
```

---

## §5 验证方案

### 5.1 迁移前数据快照

**目标**: 确保迁移前后数据一致性

```bash
# 1. 备份 TOML 文件
cp ~/.jev-switch/providers.toml ~/.jev-switch/providers.toml.backup

# 2. 导出 TOML 配置为 JSON（便于对比）
cargo run --manifest-path rs/Cargo.toml --bin dump-config > /tmp/toml-snapshot.json
```

### 5.2 迁移后一致性检查

**检查项**:
1. ✅ 服务入口数量 = TOML 中的 model ID 去重数量
2. ✅ 路由数量 = TOML `[[routes]]` + `[router]` 总数
3. ✅ 每条路由的字段值与 TOML 一致
4. ✅ 全局配置已写入 `global_config` 表
5. ✅ 迁移标记已写入 `migrations` 表

**验证脚本**:
```rust
// rs/migrations/verify.rs
pub fn verify_migration(toml_path: &Path, db_path: &Path) -> Result<()> {
    let toml_config = Config::load(toml_path)?;
    let db_conn = Connection::open(db_path)?;
    
    // 1. 检查服务入口数量
    let toml_models: HashSet<_> = toml_config.route_edges()
        .iter()
        .map(|e| e.left.clone())
        .collect();
    let db_endpoints: HashSet<_> = db_conn
        .prepare("SELECT id FROM service_endpoints")?
        .query_map([], |row| row.get(0))?
        .collect::<Result<_, _>>()?;
    assert_eq!(toml_models, db_endpoints, "service endpoints mismatch");
    
    // 2. 检查路由数量
    let toml_routes = toml_config.route_edges().len();
    let db_routes: usize = db_conn
        .query_row("SELECT COUNT(*) FROM routes", [], |row| row.get(0))?;
    assert_eq!(toml_routes, db_routes, "routes count mismatch");
    
    // 3. 检查路由字段一致性
    for edge in toml_config.route_edges() {
        let db_edge: Option<_> = db_conn.query_row(
            "SELECT left_node, right_node, priority, match_mode
             FROM routes WHERE left_node = ? AND right_node = ?",
            (&edge.left, &edge.right),
            |row| Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i32>(2)?,
                row.get::<_, String>(3)?,
            ))
        ).optional()?;
        assert!(db_edge.is_some(), "route not found in DB: {:?}", edge);
    }
    
    println!("✓ Migration verification passed");
    Ok(())
}
```

### 5.3 集成测试

**测试场景**:
1. 空配置迁移（无 `[[routes]]`，只有 `[router]`）
2. 完整配置迁移（包含 prefix、sticky、on_error）
3. 重复迁移（幂等性：第二次迁移应跳过）
4. 迁移后 API 调用（验证路由逻辑正常）

---

## §6 回滚方案

### 6.1 迁移失败时的恢复步骤

**场景**: 迁移过程中出错（如数据库写入失败、字段映射错误）

**恢复流程**:
1. **自动回滚**（事务保护）:
   ```rust
   let tx = conn.transaction()?;
   // 执行迁移...
   if let Err(e) = migrate(&tx) {
       tx.rollback()?;
       return Err(e);
   }
   tx.commit()?;
   ```

2. **删除数据库文件**:
   ```bash
   rm ~/.jev-switch/jev-switch.db
   ```

3. **恢复 TOML 备份**:
   ```bash
   cp ~/.jev-switch/providers.toml.backup ~/.jev-switch/providers.toml
   ```

4. **重启 daemon**（自动重新尝试迁移）

### 6.2 迁移后发现问题的回滚

**场景**: 迁移成功但运行时发现数据不一致

**回滚步骤**:
1. **停止 daemon**:
   ```bash
   pkill jev-switch-daemon
   ```

2. **删除数据库**:
   ```bash
   rm ~/.jev-switch/jev-switch.db
   ```

3. **恢复 TOML**:
   ```bash
   mv ~/.jev-switch/providers.toml.backup ~/.jev-switch/providers.toml
   ```

4. **重启 daemon**:
   ```bash
   JEV_SWITCH_CONFIG=~/.jev-switch/providers.toml cargo run --manifest-path rs/Cargo.toml
   ```

### 6.3 保留 TOML 作为只读备份

**策略**:
- 迁移成功后，TOML 文件保留但**不再写入**
- admin `PUT /v1/admin/routes` 只写数据库，不写 TOML
- TOML 文件重命名为 `providers.toml.pre-migration`（避免混淆）

**好处**:
- 回滚时可直接使用 TOML
- 保留原始配置作为审计记录

---

## §7 依赖与工具

### 7.1 Rust 依赖

**新增 crate**:
```toml
# rs/Cargo.toml
[workspace.dependencies]
rusqlite = { version = "0.32", features = ["bundled"] }
tokio = { version = "1", features = ["full"] }
```

**说明**:
- `rusqlite`: SQLite 驱动（`bundled` 特性：内嵌 SQLite，无需系统依赖）
- `tokio`: 异步运行时（已有，用于清理任务）

### 7.2 迁移脚本位置

```
rs/
├── migrations/
│   ├── 001_initial_schema.sql       # SQL 迁移脚本
│   ├── 002_migrate_from_toml.rs     # Rust 迁移代码
│   └── verify.rs                    # 验证脚本
├── crates/
│   └── jev-switch-daemon/
│       └── src/
│           ├── db.rs                # 数据库模块
│           ├── migrate.rs           # 迁移逻辑
│           └── main.rs              # 启动入口
```

### 7.3 数据库文件位置

**路径**: `~/.jev-switch/jev-switch.db`（与 `providers.toml` 同目录）

**权限**: 0600（与 TOML 文件一致）

---

## §8 时间估算

| 阶段 | 任务 | 工作量 |
|------|------|-------|
| **Phase 1** | Schema 设计 + SQL 脚本 | 2h |
| **Phase 2** | Rust 迁移逻辑（从 TOML 读取 + 写入数据库） | 4h |
| **Phase 3** | daemon 加载逻辑（启动时检测 + 从数据库加载） | 3h |
| **Phase 4** | 清理任务（定时删除旧日志） | 2h |
| **Phase 5** | 验证脚本 + 集成测试 | 3h |
| **Phase 6** | 文档 + 回滚方案 | 1h |
| **总计** | - | **15h** |

---

## §9 风险与缓解

| 风险 | 可能性 | 影响 | 缓解措施 |
|------|-------|------|---------|
| 迁移过程中数据丢失 | 低 | 高 | 事务保护 + TOML 备份 + 验证脚本 |
| 字段映射错误 | 中 | 中 | 单元测试 + 验证脚本 + 人工审查 |
| 数据库文件损坏 | 低 | 高 | 定期备份 + SQLite `PRAGMA integrity_check` |
| 迁移性能问题 | 低 | 低 | 批量插入（一次事务插入所有路由） |
| 旧 TOML 与数据库不一致 | 中 | 中 | 迁移后重命名 TOML（避免混淆） |

---

## §10 验收清单

### 10.1 功能验收

- [ ] daemon 启动时自动检测数据库文件
- [ ] 首次启动执行初始化迁移（创建表 + 从 TOML 导入）
- [ ] 第二次启动跳过迁移（幂等性）
- [ ] 服务入口数量 = TOML model ID 去重数量
- [ ] 路由数量 = TOML `[[routes]]` + `[router]` 总数
- [ ] 路由字段值与 TOML 一致（left/right/priority/match/sticky/on_error）
- [ ] 全局配置已写入 `global_config` 表
- [ ] 迁移标记已写入 `migrations` 表

### 10.2 性能验收

- [ ] 迁移 100 条路由耗时 < 1 秒
- [ ] daemon 启动时加载配置耗时 < 100ms
- [ ] 单次路由查询耗时 < 1ms

### 10.3 可靠性验收

- [ ] 迁移失败时自动回滚（事务保护）
- [ ] 验证脚本检测数据不一致时报错
- [ ] 数据库文件权限为 0600（Unix）
- [ ] 清理任务定期删除旧日志（默认 30 天）

### 10.4 兼容性验收

- [ ] 迁移后 TOML 文件保留（只读备份）
- [ ] 旧配置可通过删除数据库回滚到 TOML
- [ ] 支持 Windows / macOS / Linux

---

## §11 后续扩展

### 11.1 Phase 2: 服务入口配置 UI

- 实现 `/v1/admin/endpoints` API（GET/POST/PUT/DELETE）
- 前端「入口」tab（卡片布局 + 编辑模态框）
- 策略配置（failover/race/负载均衡/影子模式）

### 11.2 Phase 3: 调用统计可视化

- 实现 `/v1/admin/stats/routes` API（返回最近 24h 调用统计）
- SSE 推送实时调用事件
- 前端 Routing 页面显示线条粗细（对数映射调用次数）

### 11.3 Phase 4: 数据库备份与恢复

- 定期备份数据库文件（每天 1 次）
- 提供 `jev-switch backup` / `jev-switch restore` CLI 命令
- 支持导出为 JSON / 导入从 JSON

---

**文档作者**: Claude Opus 4.8  
**最后更新**: 2026-09-24  
**下一步**: 实现 SQL 迁移脚本（`rs/migrations/001_initial_schema.sql`）
