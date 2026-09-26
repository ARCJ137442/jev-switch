# Jev-Switch 完整实施计划
**日期**: 2026-09-24  
**基于**: UI-REDESIGN-v2.md 设计稿 + 需求对齐会话

> **后续施工入口（2026-09-24）：**见 [入口网关认知对齐与实施计划](ENDPOINT-GATEWAY-ALIGNMENT-PLAN.md)。本文保留先前 SSE、Token 分级、统计等专项；旧三页导航草图及任何“完成”声明不能覆盖最新用户裁决与运行核查。先统一提供商接入配置、对外入口及 DAG 的配置/执行，再接两类入口横比、多语言与同版本应用验收，正式截图和发布收尾最后进行。

> **历史文档提示（2026-09-27）：**本文是早期实施草案，阶段、勾选项、时间估算、CI/CD 与端到端验收清单均为当时计划，不代表当前进度或已验证能力。当前实现与缺口以 [入口网关主计划](ENDPOINT-GATEWAY-ALIGNMENT-PLAN.md) 为准，正式版本与发版步骤以 [发版手册](../RELEASE.md) 为准。下文 `v0.6.0` tag 命令仅作为历史草案记录，禁止照抄执行。

---

## 核心决策摘要

| 决策项 | 裁决结果 | 说明 |
|--------|----------|------|
| UI 架构 | Dashboard + Control + Logs 三页 | Home 是旧称，Dashboard 是新定位 |
| Playground | 保留独立页 | 不做浮层化 |
| 实时推送 | SSE (Server-Sent Events) | 阻塞 Dashboard 开发，最小可用先行 |
| Mode 切换 | 需要确认框 + 提示功能变化 | 文案提示 local/cloud 核心差异 |
| 用户叙事 | 完整旅程地图 + 故事性案例 | 两类文档各有侧重 |
| 权限方案 | 单管理员密码 + API token 分级 | 双角色 Dashboard（管理员全量，用户看自己） |
| 统计需求 | 流量监控 + 成本核算 + 权限审计 | 三者都要，数据隔离 |
| 会话管理 | 无状态 Bearer token | HTTPS 是基础操作 |

---

## Phase 1: 后端 SSE 接口（阻塞前端开发）

### 1.1 最小可用 SSE 端点

**目标**: 实现 `GET /v1/admin/events` SSE 端点，推送基础请求事件

**接口契约**:
```rust
// GET /v1/admin/events
// Content-Type: text/event-stream
// 
// 事件格式:
// event: request
// data: {"timestamp":"2026-09-24T14:32:05Z","model":"jev","upstream":"laya/jev-english","latency_ms":68,"cost":0.0002,"status":"success"}
// 
// event: failover
// data: {"timestamp":"2026-09-24T14:31:58Z","from":"laya/jev-english","to":"vercel/typesafe","reason":"503 Service Unavailable"}
```

**实施步骤**:
1. 后端新增 `EventBus`（内存队列，bounded channel）
2. 路由层每次请求完成后发送事件到 EventBus
3. `/v1/admin/events` 端点订阅 EventBus，转换为 SSE 流
4. 测试：启动后端 → curl -N 验证 SSE 流 → 手动调用 /v1/systemone 触发事件

**扩展性要求**:
- EventBus 支持多订阅者（为后续 Tauri 桌面端预留）
- 事件结构预留 `event_type` 字段（request/failover/error/config_change）
- Tauri 端和云部署模式都要测试

**验收标准**:
- [ ] curl -N http://127.0.0.1:11435/v1/admin/events 持续输出事件流
- [ ] 调用 /v1/systemone 后 1 秒内在 SSE 流中看到 request 事件
- [ ] failover 场景能看到 failover 事件
- [ ] cargo test 通过

---

## Phase 2: 权限系统（单管理员 + token 分级）

### 2.1 后端 token 配置

**新增配置段** (`rs/config.toml`):
```toml
[auth]
admin_password_hash = "..." # 已有

[[tokens]]
id = "user-alice"
name = "Alice"
key_hash = "..."
readonly = true
enabled = true

[[tokens]]
id = "admin-bob"
name = "Bob"
key_hash = "..."
readonly = false
enabled = true
```

**实施步骤**:
1. 定义 `Token` 结构（id/name/key_hash/readonly/enabled）
2. 配置加载时解析 `[[tokens]]` 数组
3. `/v1/systemone` 支持可选 `Authorization: Bearer <token>`
4. `/v1/admin/*` 要求 admin 密码 **或** 非只读 token（双重认证）

### 2.2 前端双角色 Dashboard

**登录态区分**:
- **管理员模式**: 输入 admin 密码 → 全权限，看全量数据
- **用户模式**: 输入 API token → 只读，只看该 token 的数据

**实施步骤**:
1. 新增 `LoginPage`（判断输入是密码还是 token）
2. 登录成功后存 `localStorage.setItem('auth', {type: 'admin'|'user', token: '...'})`
3. Dashboard/Logs 页根据 `auth.type` 决定：
   - 管理员：显示全量数据 + Token 管理页
   - 用户：只显示该 token 的数据，隐藏 Token 管理页
4. 所有 API 请求带 `Authorization: Bearer <token>`

**Token 管理页** (仅管理员可见):
- 卡片列表（类似 Providers 页）
- 显示：token ID / name / readonly / enabled / 最近调用时间
- 操作：创建 / 禁用 / 删除 / 查看统计

### 2.3 统计功能（三合一）

**后端新增接口**:
```rust
// GET /v1/admin/stats/by-token?from=<ISO8601>&to=<ISO8601>
// 返回:
// {
//   "tokens": [
//     {
//       "token_id": "user-alice",
//       "total_requests": 1234,
//       "total_cost": 5.67,
//       "avg_latency_ms": 89,
//       "error_rate": 0.02
//     }
//   ]
// }

// GET /v1/stats/my (用户用自己的 token 查询)
// 返回: 同上，但只有该 token 的数据
```

**Dashboard 新增卡片**（管理员可见）:
- **Top Tokens**: 显示最活跃的 5 个 token
- 点击跳转 Logs 页，过滤该 token 的请求

**Logs 页扩展**:
- 新增「按 Token 统计」tab
- 表格列：Token ID / 调用次数 / 总成本 / 平均延迟 / 错误率
- 管理员看全量，用户只看自己

---

## Phase 3: 用户叙事文档（完整旅程地图）

### 3.1 新建文档结构

```
docs/user-journeys/
├── USER-PERSONAS.md         # 三类画像（开发者/运维/爱好者）
├── USER-JOURNEYS.md         # 三类旅程（首次配置/日常监控/故障排查）
├── DECISION-TREE.md         # 用户在每个页面的决策流
└── KIRO-STORY.md            # 改写原有「Kiro 的第一天」，匹配新三页架构
```

### 3.2 内容要点

**USER-PERSONAS.md**:
- 开发者 Kiro：个人项目，追求性能和配置灵活性
- 运维 Alex：团队共享实例，关注稳定性和成本
- 爱好者 Sam：体验 Jev 协议，快速上手

**USER-JOURNEYS.md**:
- 首次配置：从安装到第一次调用成功
- 日常监控：查看 Dashboard，调整 provider 权重
- 故障排查：failover 告警 → 查 Logs → 禁用故障 provider

**DECISION-TREE.md**:
- Dashboard 页：用户看到什么 → 下一步操作选项（切 mode / 加 provider / 查日志）
- Control 页：两个 tab 的决策路径
- Logs 页：过滤 → 查看详情 → 导出 CSV

---

## Phase 4: CI/CD 自动发版

### 4.1 CI 流水线

**`.github/workflows/ci.yml`** (每次 push 自动触发):
```yaml
name: CI
on: [push, pull_request]
jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Rust 测试
        run: cargo test --manifest-path rs/Cargo.toml
      - name: 前端测试
        run: |
          cd ui
          npm ci
          npm run lint
          npm run build
```

### 4.2 自动发版流水线

**`.github/workflows/release.yml`** (打 tag 时触发):
```yaml
name: Release
on:
  push:
    tags:
      - 'v*'
jobs:
  build-tauri:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v4
      - name: Build Tauri App
        run: |
          # 构建 Windows Tauri APP
          # 输出: jev-switch-v*.msi
      - name: Upload Release Asset
        uses: actions/upload-release-asset@v1
        
  build-docker:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Build Docker Image
        run: docker build -t jev-switch:${{ github.ref_name }} .
      - name: Push to GitHub Container Registry
        run: docker push ghcr.io/${{ github.repository }}:${{ github.ref_name }}
```

**历史草案中的触发方式（已失效，不可执行）**：当时曾以 `v0.6.0` 举例。实际版本号、发布门禁和 tag 流程统一查阅 [`docs/RELEASE.md`](../RELEASE.md)，不得从本历史草案复制版本命令。

---

## Phase 5: 文档同步（根据契约调整）

### 5.1 需要更新的文档

| 文档 | 更新内容 |
|------|----------|
| `docs/08-CONTRACT` | 新增权限系统契约、SSE 接口契约 |
| `docs/contracts/05-http.md` | 补充 `/v1/admin/events` SSE 端点、`/v1/admin/stats/by-token` 接口 |
| `docs/contracts/06-frontend.md` | 更新三页架构（Dashboard/Control/Logs），补充双角色登录 |
| `docs/deployment.md` | 补充云部署模式、HTTPS 配置、token 管理 |
| `CLAUDE.md` | 补充权限模型速查、CI/CD 策略、产品定位 |

### 5.2 产品定位（一句话）

**Jev-Switch 定位**: 轻量级、本地云端双兼容、快速配置与高度可集成的 Jev 路由器，侧重性能与用户友好（界面和配置流畅度），应对大型中转站和 CC Switch 尚未支持 Jev API 管理的生态位。

---

## 验收清单

**后端**:
- [ ] SSE 接口可用（curl -N 验证）
- [ ] Token 配置加载正确
- [ ] 统计接口返回正确数据
- [ ] cargo test 全部通过

**前端**:
- [ ] Dashboard 实时流水展示
- [ ] 双角色登录（管理员/用户）
- [ ] Token 管理页（仅管理员）
- [ ] Logs 页按 token 统计
- [ ] npm run build 通过

**文档**:
- [ ] 用户旅程地图完整
- [ ] 所有 contracts/ 更新
- [ ] CLAUDE.md 补充完整

**CI/CD**:
- [ ] 每次 push 自动测试
- [ ] 打 tag 后自动构建 Tauri + Docker
- [ ] Release 页有 msi 和 docker 镜像

**端到端验证**:
- [ ] 本地启动后端 + 前端，完整走一遍用户旅程
- [ ] Tauri 端测试 SSE 推送
- [ ] 云部署模式测试（HTTPS + token 认证）
- [ ] 截图放到 awesome-jev issue 预览

---

**实施顺序**: Phase 1 → Phase 2 → Phase 3 → Phase 4 → Phase 5

**预计时间**: Phase 1-2（2-3 天）→ Phase 3-4（1-2 天）→ Phase 5（1 天）
