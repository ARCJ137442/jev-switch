# 用户叙事文档：Jev-Switch 使用旅程

**版本**: v0.5.1  
**日期**: 2026-09-24  
**目标读者**: 开发者、技术决策者、运维工程师

---

## 一、用户画像

### 画像 1：独立开发者 Alex
- **背景**: 正在构建基于 Jev 的决策系统，需要在多个上游间灵活切换
- **痛点**: Vercel AI Gateway 有时不稳定，希望有本地 Laya 作为 failover
- **目标**: 零配置快速启动，轻量级本地路由

### 画像 2：团队技术负责人 Morgan
- **背景**: 团队 5 人，需要共享 Jev 接入点，统一管理上游配额
- **痛点**: 不想每个人都配置自己的 API key，希望有中心化路由
- **目标**: 云端部署，token 分级访问控制

### 画像 3：AI 产品运维 Taylor
- **背景**: 生产环境运行多个 Jev 应用，需要监控路由健康状态
- **痛点**: 上游偶尔超时，希望实时看到 failover 情况
- **目标**: Dashboard 实时流水，快速诊断问题

---

## 二、核心旅程地图

### 旅程 1：5 分钟快速启动（Alex 的第一天）

**场景**: Alex 克隆了仓库，想快速测试 Jev-Switch

#### 步骤

1. **启动后端**（2 分钟）
```bash
# 使用示例配置
export JEV_SWITCH_CONFIG=rs/providers.example.toml
cargo run --manifest-path rs/Cargo.toml
# → 监听 127.0.0.1:11435
```

2. **打开前端**（1 分钟）
```
浏览器访问 http://127.0.0.1:11435
自动加载内置 UI（ui/dist）
```

3. **查看 Dashboard**（1 分钟）
   - 看到 2 个 provider（laya, vercel）
   - 看到 8 条路由配置
   - mode=local（本地模式，无需密码）

4. **测试 Playground**（1 分钟）
   - 选择 model: `jev`
   - 输入一个 Choice 问题
   - 点击 "Run Jev"，看到路由到 vercel

**结果**: ✅ 5 分钟内完成首次调用，零配置门槛

---

### 旅程 2：配置自己的上游（Alex 的第二天）

**场景**: Alex 想添加自己的 Vercel API key

#### 步骤

1. **进入 Providers 页面**
   - 点击 vercel 卡片的 "Replace key"
   - 粘贴 `VERCEL_TOKEN`
   - 点击 "Probe" 测试连通性

2. **添加新路由**
   - 进入 Routing 页面
   - 从左侧 `my-model` 拖出箭头
   - 连接到右侧 vercel 的 `typesafe-ai/jev` 端口
   - 保存

3. **验证路由**
   - Playground 选择 `my-model`
   - Run Jev，看到请求成功

**关键交互**: 电路板拖拽建边，直观易懂

---

### 旅程 3：切换到团队模式（Morgan 的部署日）

**场景**: Morgan 要把 Jev-Switch 部署到云端，给团队 5 人使用

#### 步骤

1. **切换 mode**
   - Dashboard 点击 "Switch to cloud mode"
   - 设置 admin 密码（首次激活）
   - 自动 rebind 到 `0.0.0.0:11435`

2. **生成调用 token**
   - Admin 面板 → Tokens
   - 创建 3 个 token（dev/staging/prod）
   - 分发给团队成员

3. **配置 failover**
   - Routing 页面
   - 设置 `jev` 模型的 priority：
     - vercel (priority=10, sticky=session)
     - laya (priority=30, on_error=next)
   - 第一次调用命中 vercel，失败后自动 failover 到 laya

**关键价值**: 一键切换本地/云端，零重启

---

### 旅程 4：实时监控与诊断（Taylor 的运维日）

**场景**: 生产环境 vercel 偶尔超时，Taylor 需要快速定位

#### 步骤

1. **Dashboard 实时流水**
   - 看到最近 50 条请求
   - 发现多条 `error: upstream timeout (vercel)`
   - 自动 failover 到 laya

2. **Providers 健康检查**
   - vercel 显示红灯（498ms → timeout）
   - laya 绿灯（1ms 正常）

3. **调整路由权重**
   - Routing 页面
   - 临时降低 vercel priority
   - 观察流水，确认流量切到 laya

**关键价值**: 实时可观测性，快速响应故障

---

## 三、关键交互细节

### 电路板焊接隐喻（Routing 页面）

**设计语言**: 
- 左侧 = API 调用点（焊点）
- 右侧 = 上游出口点（焊点）
- 箭头 = 电路连线（单向流动）

**操作流程**:
1. 点击左侧 model 端口，拖出箭头
2. 悬停到右侧 provider 内部模型端口
3. 箭头吸附（磁吸效果），释放鼠标完成连接
4. 自动保存到配置文件

**视觉反馈**:
- 拖拽时：虚线预览
- 命中端口：高亮 + 磁吸
- 完成连接：实线箭头 + priority 标签

---

### 热模式切换（Dashboard）

**操作路径**: Dashboard → mode 卡片 → "Switch to cloud"

**系统行为**:
1. 弹出密码确认框（首次激活）
2. 写入配置文件 `mode = "cloud"`
3. 更新 AuthState RwLock（内存态）
4. 触发 ListenSupervisor 热 Rebind（0.0.0.0:11435）
5. **零进程重启**，在途请求不中断

**用户感知**: 3 秒内完成切换，Dashboard 显示新 mode

---

### Provider 探测（Providers 页面）

**触发**: 点击 "Probe" 按钮

**后端行为**:
```rust
POST /v1/admin/providers/{id}/probe
→ 发送测试 Jev 请求到上游
→ 测量延迟（latency_ms）
→ 返回 {ok, latency_ms, status, error}
```

**前端反馈**:
- 探测中：按钮 spinner
- 成功：绿灯 + 延迟数字（1ms）
- 失败：红灯 + 错误信息（已脱敏）

---

## 四、典型问题与解决方案

### Q1: 如何快速测试 failover？

**方案**: Playground 页面
1. 选择配置了 failover 的 model（如 `jev`）
2. 故意关闭 priority=10 的上游（vercel）
3. Run Jev，观察 Dashboard 流水
4. 看到 "failover: vercel → laya"

---

### Q2: 本地模式下如何保护配置文件？

**方案**: 配置文件权限（Unix）
```bash
chmod 600 rs/providers.example.toml
```
启动时 daemon 会检查并告警（Windows 降级 warning）

---

### Q3: 如何在生产环境部署？

**方案**: Docker + cloud mode
```bash
docker build -t jev-switch .
docker run -d -p 11435:11435 \
  -e JEV_SWITCH_MODE=cloud \
  -e ADMIN_PASSWORD=<secret> \
  jev-switch
```

配置文件挂载到 `/app/config.toml`

---

## 五、进阶场景

### 场景 1：多级 failover 链

**需求**: vercel → laya → 本地模型

**实现**: Routing 页面配置 3 条路由
```
jev → vercel (priority=10, on_error=next)
jev → laya (priority=20, on_error=next)
jev → local-model (priority=30, on_error=fail)
```

**行为**: 按 priority 递增顺序尝试，直到成功或全失败

---

### 场景 2：基于 model 前缀的路由

**需求**: `local/*` 模型全部路由到 laya

**实现**: Routing 页面配置 prefix 路由
```
left: local/*
match: prefix
right: laya
upstream_model: null (透传)
```

**行为**: `local/jev` → laya `/v1/systemone` (model=local/jev)

---

### 场景 3：Session sticky（同一会话钉死上游）

**需求**: 用户首次调用选中 vercel 后，后续调用保持在 vercel

**实现**: Routing 配置
```
jev → vercel (priority=10, sticky=session)
jev → laya (priority=20)
```

**行为**: 首次随机选中后，后续通过 session cookie 钉死

---

## 六、成功指标

用户完成以下里程碑，说明 Jev-Switch 满足需求：

| 里程碑 | 描述 | 目标时间 |
|--------|------|---------|
| 首次启动 | 从克隆到首次调用成功 | 5 分钟 |
| 配置上游 | 添加自己的 API key + 新路由 | 10 分钟 |
| 模式切换 | local → cloud，理解双态差异 | 5 分钟 |
| Failover 验证 | 观察到自动故障转移 | 3 分钟 |
| 生产部署 | Docker 部署 + token 分发 | 30 分钟 |

---

**文档作者**: Claude Opus 4.8  
**最后更新**: 2026-09-24
