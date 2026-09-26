# 服务入口配置功能完整实施计划

**日期**: 2026-09-24  
**版本**: 1.0（基于 super-questioning + popup-ask 深度对齐）  
**状态**: 历史专项方案保留；按最新裁决实施中，尚未完成执行闭环验收
**作者**: Claude Opus 4.8

> **2026-09-24 用户补充裁决（GPT-6 Astra 整理）：**本专项按 [入口网关认知对齐与实施计划](ENDPOINT-GATEWAY-ALIGNMENT-PLAN.md) 接续。本文“服务入口/入口卡片”专指 **Jev Switch 对外提供的模型服务**；上游账号、key 与模型资源由提供商复合卡片管理。一份接入配置一张卡，同地址不同账号可分卡。最终路由图是左侧对外入口卡片到右侧提供商模型端口的可编辑调用 DAG。本文保留历史策略与配置 API 草图，实施细节按 [契约 07](../contracts/07-入口网关修订.md) 修订；执行闭环尚未验收，不能按接口存在判定完成。

---

## §0 功能定位（一句话）

**服务入口配置是 Jev-Switch 的亮点功能 — 对外暴露一个模型 ID，对内配置一个路由策略（failover/race/负载均衡/影子），支持实时切换，无需重启。**

---

## §1 核心概念

### 1.1 服务入口 (Service Endpoint)

**内部管理定义**: 一个对外服务入口 = 一个对外暴露的模型 ID + 一组路由 + 一个路由策略。

**调用者视角**: `网关地址 + 调用 token + 对外模型 ID`。本地调用 token 通常为空；不同调用 token 访问同一入口时共享该入口的路由规则，不按 token 覆盖策略。对外模型 ID 与内部入口 ID 可以使用同一个值。

**与上游的关系**: 上游调用入口由具体提供商接入配置（地址、账号/key）及一个模型确定。多个对外入口可以复用同一上游入口；每个对外入口独立维护自己的路由与策略。上游模型集合不自动等于对外已发布入口集合。

**示例**:
```
入口 ID: jev
策略: failover
路由:
  - jev → vercel/typesafe-ai/jev (priority: 10)
  - jev → laya/laya-english (priority: 20)
```

**语义**:
- 客户端调用 `POST /v1/systemone` 时，`model` 字段填 `jev`
- 后端根据 `jev` 入口的策略（failover），按 priority 顺序尝试路由
- 若 vercel 失败，根据 `on_error` 决定是否尝试 laya

### 1.2 路由策略 (Routing Strategy)

| 策略 | 说明 | 适用场景 |
|------|------|---------|
| **Failover**（默认） | 按 priority 顺序尝试，第一条成功即返回；失败后根据 `on_error` 决定是否尝试下一条 | 高可用场景（Vercel 挂了切 Laya） |
| **Race** | 同时发送给所有路由，取最快响应 | 低延迟场景（多上游竞速） |
| **Load Balancing** | 按 priority 权重随机选择一条路由 | 负载均衡场景（分散流量） |
| **Shadow** | 主路由返回结果，同时向影子路由发送请求（不等待响应） | A/B 测试场景（对比上游） |

### 1.3 配置粒度

**全局默认 + model 级覆盖**:
- 全局默认策略：`failover`（适用于所有未指定策略的入口）
- Model 级覆盖：每个入口可以选择「跟随全局」或「特定配置」

**枚举定义**:
```rust
enum StrategyConfig {
    FollowGlobal,           // 跟随全局配置
    Specific(Strategy),     // 特定配置（failover/race/...）
}
```

---

## §2 对齐结果总览

### 2.1 核心设计决策（12 个）

| # | 维度 | 决策结果 |
|---|------|---------|
| 1 | **存储位置** | 内存 + SQLite 持久化（对标 CC Switch） |
| 2 | **生效机制** | 实时生效（API 修改后下一个请求立即使用新策略） |
| 3 | **配置粒度** | 全局默认（failover）+ model 级覆盖（跟随全局 / 特定配置） |
| 4 | **UI 布局** | 「入口」tab（Routing 页面顶栏左侧），卡片式布局 |
| 5 | **卡片内容** | 模型 ID + 策略 + 路由数量 + 健康圆环图 + 调用统计（24h/总） |
| 6 | **卡片操作** | 编辑 + 删除 + 启用/禁用 + 复制模型 ID（点击后图标变 ✓） |
| 7 | **模态框内容** | 三层布局：基础配置（默认展开）+ 路由管理（默认展开）+ 高级参数（默认收起） |
| 8 | **模型 ID 可编辑** | 入口本质是服务入口，不是固定的模型 ID；修改后更新相关数据 |
| 9 | **高级参数** | 基础参数（race 超时 + 负载均衡权重），突出可扩展性 |
| 10 | **默认行为** | 手动创建（+ 按钮），创建时可指定路由；不指定时只添加入口 |
| 11 | **数据库迁移** | fork subAgent 负责设计 SQLite schema + 迁移脚本 |
| 12 | **持久化时机** | 立即持久化（每次 API 修改后立即写入数据库） |

### 2.2 UI 参考资源

| 参考页面 | 用途 | URL |
|---------|------|-----|
| **API 密钥配置** | 卡片布局 + 操作按钮 | https://user.modelshare.cc/keys |
| **模型市场** | 模型卡片 + 健康状态 | https://user.modelshare.cc/market |
| **使用统计** | 统计界面 + 布局 | https://user.modelshare.cc/usage |

**学习方式**: 用 CDP (Chrome DevTools Protocol) 或 `/web-access` skill 学习组件布局

---

## §3 数据库设计

### 3.1 Schema 设计

#### 表 1: `service_endpoints`（服务入口）

| 字段 | 类型 | 约束 | 说明 |
|------|------|------|------|
| `id` | TEXT | PRIMARY KEY | 入口 ID（对外暴露的模型 ID） |
| `strategy_config` | TEXT | NOT NULL | 策略配置：`"follow_global"` 或 JSON 格式的特定策略 |
| `enabled` | INTEGER | NOT NULL DEFAULT 1 | 启用状态（0=禁用，1=启用） |
| `created_at` | INTEGER | NOT NULL | 创建时间戳（Unix timestamp） |
| `updated_at` | INTEGER | NOT NULL | 更新时间戳（Unix timestamp） |

**`strategy_config` 示例**:
```json
// 跟随全局
"follow_global"

// 特定配置
{
  "type": "failover"
}

{
  "type": "race",
  "params": {
    "timeout_ms": 5000
  }
}

{
  "type": "load_balance",
  "params": {
    "weight_mode": "priority"
  }
}

{
  "type": "shadow",
  "params": {
    "shadow_target": "laya"
  }
}
```

#### 表 2: `routes`（路由配置）

现有路由配置保持不变，继续使用当前的存储方案（可能是 TOML 文件或数据库中的 routes 表）。

**关联关系**:
- 一个服务入口（`service_endpoints.id`）可以关联多条路由（`routes.left`）
- 查询入口的路由：`SELECT * FROM routes WHERE left = ?`

### 3.2 全局配置

**存储位置**: `global_config` 表或配置文件

| 字段 | 类型 | 默认值 | 说明 |
|------|------|-------|------|
| `default_strategy` | TEXT | `"failover"` | 全局默认策略 |

---

## §4 后端 API 设计

### 4.1 获取所有服务入口

#### GET /v1/admin/endpoints

**响应**:
```json
{
  "endpoints": [
    {
      "id": "jev",
      "strategy_config": {
        "type": "follow_global"
      },
      "enabled": true,
      "routes_count": 2,
      "health_summary": {
        "total": 2,
        "healthy": 1,
        "degraded": 1,
        "failed": 0
      },
      "calls_24h": 1234,
      "calls_total": 56789,
      "created_at": 1713542400,
      "updated_at": 1713542400
    },
    {
      "id": "laya-english",
      "strategy_config": {
        "type": "race",
        "params": {
          "timeout_ms": 3000
        }
      },
      "enabled": true,
      "routes_count": 1,
      "health_summary": {
        "total": 1,
        "healthy": 1,
        "degraded": 0,
        "failed": 0
      },
      "calls_24h": 567,
      "calls_total": 12345,
      "created_at": 1713542400,
      "updated_at": 1713542400
    }
  ],
  "global_default_strategy": "failover"
}
```

**字段说明**:
- `health_summary`: 聚合该入口所有路由的 provider 健康状态
  - `healthy`: 延迟 < 1000ms
  - `degraded`: 延迟 >= 1000ms 或部分路由失败
  - `failed`: 所有路由失败
- `calls_24h` / `calls_total`: 从调用统计 API 获取

---

### 4.2 创建服务入口

#### POST /v1/admin/endpoints

**请求**:
```json
{
  "id": "jev",
  "strategy_config": {
    "type": "follow_global"
  },
  "routes": [
    {
      "left": "jev",
      "right": "vercel",
      "upstream_model": "typesafe-ai/jev",
      "priority": 10,
      "match": "exact"
    },
    {
      "left": "jev",
      "right": "laya",
      "upstream_model": "laya-english",
      "priority": 20,
      "match": "exact"
    }
  ]
}
```

**响应**:
```json
{
  "id": "jev",
  "strategy_config": {
    "type": "follow_global"
  },
  "enabled": true,
  "created_at": 1713542400,
  "updated_at": 1713542400
}
```

**说明**:
- `routes` 字段可选：不提供时只创建入口，不创建路由
- 提供 `routes` 时，同时创建入口和路由（原子性操作）

---

### 4.3 更新服务入口

#### PUT /v1/admin/endpoints/{id}

**请求**:
```json
{
  "id": "jev-v2",  // 修改入口 ID（可选）
  "strategy_config": {
    "type": "race",
    "params": {
      "timeout_ms": 5000
    }
  },
  "enabled": true
}
```

**响应**:
```json
{
  "id": "jev-v2",
  "strategy_config": {
    "type": "race",
    "params": {
      "timeout_ms": 5000
    }
  },
  "enabled": true,
  "updated_at": 1713542500
}
```

**说明**:
- 若修改 `id`，需要同步更新所有关联路由的 `left` 字段
- 修改后立即生效（下一个请求使用新策略）

---

### 4.4 删除服务入口

#### DELETE /v1/admin/endpoints/{id}

**响应**:
```json
{
  "id": "jev",
  "deleted": true,
  "routes_deleted": 2
}
```

**说明**:
- 删除入口时，关联的路由也会被删除（级联删除）
- 需要二次确认（UI 层面）

---

### 4.5 获取全局默认策略

#### GET /v1/admin/config/default_strategy

**响应**:
```json
{
  "default_strategy": "failover"
}
```

---

### 4.6 更新全局默认策略

#### PUT /v1/admin/config/default_strategy

**请求**:
```json
{
  "default_strategy": "race"
}
```

**响应**:
```json
{
  "default_strategy": "race",
  "updated_at": 1713542600
}
```

---

## §5 前端 UI 设计

### 5.1 页面布局

**「入口」tab + 「路由」tab**（Routing 页面顶栏左侧）

```
┌─────────────────────────────────────────────────────────────┐
│ Jev-Switch  Dashboard  Control  Logs        ☀  EN  ⚙  v0.5.0│
└─────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────┐
│ [入口] [路由]                             [+ 新建入口]       │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  ┌─────────────────┐  ┌─────────────────┐  ┌──────────────┐│
│  │ jev             │  │ laya-english    │  │ vercel-gpt4o ││
│  │ failover · 2 ↗  │  │ race · 1 ↗      │  │ failover · 1 ↗│
│  │ ◉ 1/2 healthy   │  │ ◉ 1/1 healthy   │  │ ◉ 1/1 healthy││
│  │ 1,234 calls(24h)│  │ 567 calls(24h)  │  │ 89 calls(24h)││
│  │ [编辑] [⊗] [✓]  │  │ [编辑] [⊗] [✓]  │  │ [编辑] [⊗] [✓]│
│  └─────────────────┘  └─────────────────┘  └──────────────┘│
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

**卡片内容**（侧重程度排序）:
1. **模型 ID**（主标题，可编辑）
2. **策略**（副标题，如 `failover`）
3. **路由数量**（如 `2 routes` 或 `2 ↗`）
4. **健康圆环图**（◉，红/黄/绿 = 不可用/慢/正常）+ 状态文字（如 `1/2 healthy`）
5. **调用统计**（如 `1,234 calls(24h)` + `56,789 total`，hover 显示总计）

**卡片操作**:
- **[编辑]**: 打开编辑模态框
- **[⊗]**: 删除入口（弹出确认对话框）
- **[✓]**: 复制模型 ID 到剪贴板（点击后图标短暂变成 ✓）
- **启用/禁用开关**: 卡片右上角，类似 provider 卡片

---

### 5.2 编辑模态框

**三层布局**（自上而下）:

```
┌───────────────────────────────────────────────────────────┐
│  编辑服务入口: jev                               [✕ 关闭]  │
├───────────────────────────────────────────────────────────┤
│                                                            │
│  【基础配置】（默认展开）                                   │
│  ┌────────────────────────────────────────────────────┐   │
│  │ 模型 ID: [jev_______________] (可编辑)              │   │
│  │ 路由策略: [下拉菜单: 跟随全局 / failover / race…]  │   │
│  │ 启用状态: [✓ 启用]                                 │   │
│  └────────────────────────────────────────────────────┘   │
│                                                            │
│  【路由管理】（默认展开）                                   │
│  ┌────────────────────────────────────────────────────┐   │
│  │ 关联路由 (2 条):                                    │   │
│  │ ┌──────────────────────────────────────────────┐   │   │
│  │ │ jev → [vercel ▼] / [typesafe-ai/jev ▼] ⚙ 10 │ [X]│  │
│  │ └──────────────────────────────────────────────┘   │   │
│  │ ┌──────────────────────────────────────────────┐   │   │
│  │ │ jev → [laya ▼] / [laya-english ▼] ⚙ 20      │ [X]│  │
│  │ └──────────────────────────────────────────────┘   │   │
│  │ [+ 添加路由]                                        │   │
│  └────────────────────────────────────────────────────┘   │
│                                                            │
│  【高级参数】（默认收起，点击展开）▼                        │
│  （根据策略动态显示）                                       │
│  ┌────────────────────────────────────────────────────┐   │
│  │ race 策略参数:                                      │   │
│  │ 超时时间: [5000] ms                                 │   │
│  └────────────────────────────────────────────────────┘   │
│                                                            │
│                                 [取消]  [保存并应用]       │
└───────────────────────────────────────────────────────────┘
```

**路由管理交互**:
- **默认状态**: 展示格式（如 `jev → vercel / typesafe-ai/jev ⚙ 10`）
- **点击进入编辑**: 下拉选项可编辑（provider 下拉菜单 + model 下拉菜单 + priority 输入框）
- **[X] 删除**: 删除该条路由
- **[+ 添加路由]**: 新增一条路由（默认进入编辑状态）

---

### 5.3 新建入口模态框

**与编辑模态框类似**，但模型 ID 为空（必填）

**创建时可选操作**:
- 只填写模型 ID + 策略 → 创建入口（无路由）
- 填写模型 ID + 策略 + 路由 → 创建入口并添加路由

---

## §6 高级参数设计

### 6.1 基础参数（Phase 4）

| 策略 | 参数 | 类型 | 默认值 | 说明 |
|------|------|------|-------|------|
| **Failover** | 无参数 | - | - | 使用路由的 `on_error` 字段控制 failover 行为 |
| **Race** | `timeout_ms` | number | 5000 | 超时时间（毫秒），超时后取已完成的最快响应 |
| **Load Balancing** | `weight_mode` | enum | `"priority"` | 权重模式：`"priority"`（按 priority 权重）/ `"uniform"`（均匀分布） |
| **Shadow** | `shadow_target` | string | （必填） | 影子路由的 provider ID |

### 6.2 可扩展性设计

**未来可扩展的参数**（Phase 5+）:
- Failover: `max_retries`（最大重试次数）
- Race: `cancel_slower`（是否取消慢的请求）
- Load Balancing: `sticky_session`（会话保持）
- Shadow: `sample_rate`（采样率，0-1）

---

## §7 数据库迁移方案

### 7.1 subAgent 任务分配

**fork 一个 subAgent 负责**：
1. 设计 SQLite schema（`service_endpoints` 表 + `global_config` 表）
2. 编写迁移脚本（从现有 TOML 文件迁移到数据库）
3. daemon 启动时加载数据库的逻辑
4. 测试迁移流程（确保数据完整性）

**输出文档**:
- `docs/design/DATABASE-MIGRATION-PLAN.md`（迁移计划）
- `rs/migrations/001_add_service_endpoints.sql`（SQL 迁移脚本）

---

### 7.2 迁移流程

**步骤**:
1. daemon 启动时，检查 `service_endpoints` 表是否存在
2. 若不存在，执行迁移脚本：
   - 创建 `service_endpoints` 表
   - 从 `providers.toml` 读取现有配置
   - 推导所有 `model_id`（从 `routes.left` 去重）
   - 为每个 `model_id` 创建一个入口（默认策略：跟随全局）
3. 迁移完成后，写入迁移标记（`migrations` 表）

**兼容性**:
- 迁移后，TOML 文件作为只读备份
- daemon 只从数据库读取配置
- 用户可通过 API 或 UI 修改配置

---

## §8 实施计划

### 8.1 Phase 4.1: 数据库迁移（P0）

| 任务 | 负责 | 工作量 | 依赖 |
|------|------|-------|------|
| fork subAgent 设计数据库 schema | subAgent | 2h | - |
| 编写迁移脚本 | subAgent | 2h | schema 设计 |
| 实现 daemon 启动时加载数据库 | 主会话 | 1h | 迁移脚本 |
| 测试迁移流程 | 主会话 | 1h | 加载逻辑 |

**总计**: ~6 小时

---

### 8.2 Phase 4.2: 后端 API（P0）

| 任务 | 工作量 | 依赖 |
|------|-------|------|
| 实现 GET /v1/admin/endpoints | 1.5h | 数据库 schema |
| 实现 POST /v1/admin/endpoints | 2h | 路由创建逻辑 |
| 实现 PUT /v1/admin/endpoints/{id} | 2h | 实时生效机制 |
| 实现 DELETE /v1/admin/endpoints/{id} | 1h | 级联删除 |
| 实现全局默认策略 API | 0.5h | - |
| 实现策略路由逻辑（failover/race/负载均衡） | 4h | 路由引擎 |

**总计**: ~11 小时

---

### 8.3 Phase 4.3: 前端 UI（P1）

| 任务 | 工作量 | 依赖 |
|------|-------|------|
| 学习参考 UI（CDP / web-access） | 1h | - |
| 实现「入口」tab + 卡片布局 | 3h | GET /v1/admin/endpoints |
| 实现卡片内容（健康圆环图 + 调用统计） | 2h | 调用统计 API |
| 实现卡片操作（编辑/删除/启用/复制） | 2h | - |
| 实现新建/编辑模态框（三层布局） | 4h | POST/PUT API |
| 实现路由管理（可编辑下拉菜单） | 3h | - |
| 实现高级参数表单（动态显示） | 2h | - |
| i18n 国际化（中英文） | 1h | - |

**总计**: ~18 小时

---

### 8.4 Phase 4.4: 测试与验收（P1）

| 任务 | 工作量 | 依赖 |
|------|-------|------|
| 后端单元测试（策略路由逻辑） | 2h | 后端 API |
| 前端集成测试（卡片操作 + 模态框） | 2h | 前端 UI |
| 端到端验收（创建/编辑/删除入口） | 2h | 前后端 |
| 性能测试（并发策略切换） | 1h | 实时生效机制 |

**总计**: ~7 小时

---

## §9 总工作量估算

| Phase | 内容 | 工作量 |
|-------|------|-------|
| Phase 4.1 | 数据库迁移 | 6h |
| Phase 4.2 | 后端 API | 11h |
| Phase 4.3 | 前端 UI | 18h |
| Phase 4.4 | 测试与验收 | 7h |

**总计**: **42 小时**（约 5-6 个工作日）

**原估算**: 13 小时（Phase 4 亮点功能）  
**新估算**: 42 小时（完整实施 = 13h × 3.2）

**原因**: 新增了完整的数据库迁移、UI 参考学习、三层模态框布局、路由管理交互等细节。

---

## §10 验收清单

### 10.1 数据库验收

- [ ] `service_endpoints` 表创建成功
- [ ] 从 TOML 迁移到数据库，数据完整
- [ ] daemon 启动时正确加载数据库配置
- [ ] 迁移标记写入 `migrations` 表

### 10.2 后端 API 验收

- [ ] GET /v1/admin/endpoints 返回所有入口 + 健康状态
- [ ] POST /v1/admin/endpoints 创建入口 + 可选路由
- [ ] PUT /v1/admin/endpoints/{id} 修改策略，立即生效
- [ ] DELETE /v1/admin/endpoints/{id} 删除入口 + 级联删除路由
- [ ] 全局默认策略 API 正常工作
- [ ] Failover / Race / Load Balancing 策略路由正确

### 10.3 前端 UI 验收

- [ ] 「入口」tab 显示所有入口卡片
- [ ] 卡片内容完整（模型 ID + 策略 + 路由数量 + 健康圆环图 + 调用统计）
- [ ] 卡片操作正常（编辑/删除/启用/复制）
- [ ] 新建入口模态框可创建入口 + 可选路由
- [ ] 编辑模态框三层布局（基础/路由/高级）
- [ ] 路由管理可编辑（下拉菜单 + 添加/删除）
- [ ] 高级参数根据策略动态显示
- [ ] 模型 ID 可编辑，修改后同步更新路由
- [ ] 中英文国际化完整

### 10.4 集成验收

- [ ] 创建入口 → UI 立即显示新卡片
- [ ] 修改策略 → 下一个请求立即使用新策略
- [ ] 删除入口 → 卡片消失 + 路由级联删除
- [ ] 切换全局默认策略 → 所有「跟随全局」的入口生效
- [ ] 并发修改策略 → 无数据竞争（RwLock 保护）

---

## §11 风险与缓解

| 风险 | 可能性 | 影响 | 缓解措施 |
|------|-------|------|---------|
| 数据库迁移失败 | 中 | 高 | subAgent 负责设计 + 充分测试 + 备份 TOML |
| 实时生效并发冲突 | 中 | 中 | 使用 `RwLock` 保护策略表 + 单元测试 |
| UI 参考学习耗时 | 低 | 低 | 限制学习时间（1h），不过度模仿 |
| 工作量超预期 | 高 | 中 | 优先实现 P0 功能，高级参数推迟到 Phase 5 |

---

## §12 下一步

1. **立即**: fork subAgent 负责数据库迁移方案（输出 DATABASE-MIGRATION-PLAN.md）
2. **Phase 4.1**: 等待 subAgent 完成数据库设计 + 迁移脚本
3. **Phase 4.2**: 实现后端 API（并行：策略路由逻辑）
4. **Phase 4.3**: 实现前端 UI（学习参考 UI → 卡片布局 → 模态框）
5. **Phase 4.4**: 测试与验收

---

**文档作者**: Claude Opus 4.8  
**最后更新**: 2026-09-24  
**下一步**: fork subAgent 设计数据库 schema
