# 本地模型提供商生命周期计划

**状态：**0.10.x 已实现 provider 级结构化 argv、双重 host-command/controllable 门控、readiness、startup_check/on_demand 前置、provider 互斥、生命周期所有权快照、独立审计表、admin lifecycle domain API、主机命令安全开关和 Provider 高级 UI；Windows Laya/StartLux 已通过本机 EXE API 的启动、就绪、实际请求和停止验收。细粒度 Agent scope、跨 daemon 的安全进程组回收、OneJev/OpenJev 与其他平台验收仍未完成。
**适用版本：**`0.10.x` 生命周期主线；模型画像/属性路由移至 `0.11.0`，不属于 `v0.9.3` 能力。  
**目标：**在保持 Jev-Switch 作为路由网关的边界下，为已登记的本地模型提供商增加可控的启动、停止和状态检查能力。

## 背景与边界

当前 provider 的 `enabled` 只表示“允许路由使用”，并不代表本地模型进程正在运行。用户仍需分别打开 StartLux、OpenJev、Laya 或其他本地服务，再回到 Jev-Switch 配置地址和模型。这使本地服务管理与路由管理割裂。

本计划增加的是“provider 关联的外部服务生命周期配置”，不是通用终端，也不是模型权重管理器：

- Jev-Switch 不下载、加载、卸载模型权重，不猜测厂商进程结构。
- provider 的路由启用状态与外部服务运行状态始终分开显示。
- 只有用户明确登记的 provider 生命周期配置才能执行；不接受远程任意 shell 字符串。
- 主机命令执行由全局 `allow_host_commands` 和 provider 的 `controllable` 双重门控；两者任一关闭都不得执行命令。
- local、LAN、cloud 和 Android 都可以承载此能力，但默认关闭；桌面 LAN 开放时仅 loopback 仍视为本机所有者，其他 LAN/cloud 请求必须经过管理员权限，Android 只在 local 模式并经用户明确开启后允许。
- 没有生命周期配置的 provider 保持现有手工管理行为，完全兼容旧配置。

## 四维状态模型

Provider 的四个维度必须分别建模、分别授权、分别呈现：

| 维度 | 字段/状态 | 说明 |
|---|---|---|
| 路由资格 | `enabled` | 允许/禁止将调用交给 provider；云端和本地都适用 |
| 控制能力 | `controllable` | 是否允许 Jev-Switch 使用生命周期配置控制服务；默认 `false` |
| 服务事实 | `service_state` | `running`、`starting`、`stopped`、`unknown`、`failed`；来自状态命令和 readiness |
| 进程策略 | `process_policy` | `persistent`、`session`、`external`；决定 Jev-Switch 退出时是否处理本应用启动的进程 |

云端 provider 通常是 `controllable=false` 的不可控服务；本地 Laya/OpenJev/StartLux 等可以在用户明确开启后成为可控服务。`enabled=true` 不得隐式把 `controllable` 改为 true，也不得因为 provider 可控就自动启动。

全局设置增加 `allow_host_commands`，默认 `false`。开启它是主机命令执行的总开关，必须经过 5 秒冷却的风险模态框；关闭时所有 provider lifecycle 命令和 Agent 执行都返回结构化禁用错误。local loopback、LAN/cloud 管理员、Android local 都遵循同一总开关，不再为平台制造隐式例外。

## 用户模型

高级提供商配置增加“本地服务生命周期”折叠面板：

| 配置 | 语义 |
|---|---|
| `允许路由` | 现有 `enabled`；决定是否把请求交给 provider |
| `可控服务` | `controllable`；开启后才显示和允许生命周期命令，默认关闭 |
| `进程策略` | `persistent`（应用退出后保留）、`session`（随本应用会话结束）、`external`（外部拥有，只能调用显式停止命令） |
| `启动策略` | `manual`（默认）、`startup_check`、`on_demand` |
| `启动命令` | 可选；空值表示不由 Jev-Switch 启动 |
| `停止命令` | 可选；空值表示不由 Jev-Switch 停止，不得擅自杀进程 |
| `状态检查命令` | 可选；空值时使用 provider health/probe 作为弱状态证据 |
| `工作目录` | 可选，必须是用户明确配置的本地目录 |
| `启动等待/检查超时` | 有界时间，默认 30 秒，最大值由 daemon 限制 |

“启动/停止服务”按钮改变的是外部服务状态；“允许路由”按钮仍只改变路由资格；“可控服务”只改变 Jev-Switch 是否拥有执行这些生命周期动作的权限。三个状态必须在卡片上分别显示，例如“允许路由 / 可控服务 / 服务未运行”。

### 状态检查约定

状态命令不依赖 stdout 文案，使用退出码避免语言和日志差异：

- `0`：明确运行中。
- `3`：明确未启动；仅在 `startup_check` 或用户点击“启动”流程中允许触发启动命令。
- `4`：启动中/尚未就绪；继续有限轮询。
- 其他退出码或无法执行：未知/检查失败，不自动启动。

启动命令返回成功只代表“命令已接受”。daemon 还必须使用 provider 的已配置地址执行非推理 health/probe，并在服务身份、协议和模型可用性满足要求后才显示“运行中”。

## 命令表示与环境变量

配置在存储层使用结构化进程描述，不使用一整段 shell 文本：

```toml
allow_host_commands = false

[providers.openjev.lifecycle]
mode = "startup_check"
controllable = true
process_policy = "persistent"
program = "C:/Tools/OpenJev/openjev.exe"
args = ["--host", "127.0.0.1", "--port", "11436"]
stop_program = "C:/Tools/OpenJev/openjev-stop.exe"
stop_args = []
status_program = "C:/Tools/OpenJev/openjev-status.exe"
status_args = []
inject_api_key = false
working_dir = "C:/Tools/OpenJev"
timeout_ms = 30000
readiness_timeout_ms = 30000
```

UI 可以提供 `${JEV_PROVIDER_BASE_URL}` 等占位符便利，但 daemon 必须先解析成 argv 和环境变量，再调用 `std::process::Command`；不得把用户输入拼进 `cmd.exe /c`, PowerShell `-Command`, `sh -c` 或其他 shell。未知占位符、引号逃逸、重定向和管道语法均拒绝保存。

每次执行按需注入以下变量，子进程结束后立即释放：

| 变量 | 敏感 | 内容 |
|---|---:|---|
| `JEV_PROVIDER_ID` | 否 | provider ID |
| `JEV_PROVIDER_KIND` | 否 | `laya`、`typesafe` 等 kind |
| `JEV_PROVIDER_BASE_URL` | 否 | provider 配置的 API 地址 |
| `JEV_PROVIDER_MODELS` | 否 | 逗号分隔模型 ID，兼容简单脚本 |
| `JEV_PROVIDER_MODELS_JSON` | 否 | JSON 数组，避免模型 ID 含逗号时歧义 |
| `JEV_PROVIDER_API_KEY` | **是** | 有效 API key，仅在用户确认且命令明确声明引用时注入 |
| `JEV_PROVIDER_API_KEY_ENV` | 否 | 配置的外部 key 环境变量名 |
| `JEV_PROVIDER_WORKING_DIR` | 否 | 规范化后的工作目录 |

默认不把完整 daemon 环境复制给子进程，只保留必要的系统 PATH/平台运行变量和上述 allowlist。API key 不写入命令历史、日志、toast、事件详情、崩溃报告或持久 lifecycle 输出。

## 安全与权限门槛

这是本地代码执行能力，配置界面必须明确显示风险：命令可以读取本机文件、启动后台进程或访问环境变量。保存/首次执行以下任一情况时需要模态确认：

模型运行时的差异由外部服务脚本承担，Jev Switch 不内置 Laya、StartLux、OneJev 的专用启动器。脚本规范与本机验证见 [`LOCAL-MODEL-SERVICE-SCRIPTS.md`](LOCAL-MODEL-SERVICE-SCRIPTS.md)。

- 命令使用 `${JEV_PROVIDER_API_KEY}` 或等价敏感变量。
- 命令程序来自 PATH 而不是用户选择的绝对路径。
- `working_dir` 位于用户配置目录之外。
- `allow_host_commands` 正在从关闭切换为开启。
- provider 运行在 LAN 开放模式、daemon 为 cloud 模式或当前设备是 Android。

默认策略：

- local + loopback：开启全局开关并确认后，允许本机管理员配置和执行。
- local + LAN 开放：默认关闭；开启时必须由管理员确认，配置和执行接口都要求管理员会话，普通调用 token 只能读取脱敏状态。
- cloud：默认关闭；只有管理员可以配置并开启，Agent 也必须持有管理员级 lifecycle scope；普通用户和调用 token 永远不能执行主机命令。
- Android local：允许用户在 APP 内明确开启；仍使用同一 5 秒警告、controllable 门控和 Agent 权限，不把 Android 的原生 service/tile 状态冒充为命令生命周期状态。
- Android LAN/cloud：默认关闭，除非后续明确设计远程管理员授权和设备绑定。

命令输出只保留截断、脱敏后的最后若干行用于本次 toast/诊断；任何命中已知 key、Bearer、`*_API_KEY` 或 secret 模式的文本都必须先 redact。命令并发使用 provider 级互斥锁，避免连续点击启动产生多个进程。

## 进程所有权与停止语义

启动方式必须记录 `execution_id`、启动时间、命令摘要 hash、PID/进程组（若平台可得）和 readiness 结果，但不记录完整命令参数中的敏感值。

- `persistent`：直接由 daemon 启动且能取得 PID/进程组的服务，在 Jev-Switch/daemon 退出时保留；只有用户显式点击停止或 Agent 获得 lifecycle execute 权限时才停止。
- `session`：直接由 daemon 启动且能取得 PID/进程组的服务，daemon 正常退出时按拥有关系停止；崩溃/断电恢复时不得凭旧 PID 强杀，重新检查身份后再决定。
- `external`：服务由用户或其他 supervisor 拥有，Jev-Switch 只调用显式停止命令；没有停止命令时“停止”按钮必须禁用并说明原因。
- 直接由 daemon 启动且能取得 PID/进程组的进程，标记为“本应用拥有”；停止时优先使用停止命令，缺失时才按 `process_policy` 结束本应用拥有的进程组。
- 不得按端口、进程名或模糊命令行匹配杀进程，避免误杀用户手工启动的 LM Studio/Python/其他项目。
- Windows 使用 Job Object/进程组边界，Unix 使用独立 process group；实现前分别做平台 spike，不能用 `kill -9` 或 `taskkill /IM` 作为通用方案。

## API 草案

Provider 日常配置使用单资源 `PUT/DELETE /v1/admin/providers/{id}`；整表批量替换不作为普通接口暴露，生命周期配置通过独立资源管理，避免 key 和命令字段混入普通 provider 视图：

```text
GET  /v1/admin/providers/{id}/lifecycle
PUT  /v1/admin/providers/{id}/lifecycle
POST /v1/admin/providers/{id}/lifecycle/status
POST /v1/admin/providers/{id}/lifecycle/start
POST /v1/admin/providers/{id}/lifecycle/stop
```

普通 provider 列表只返回结构化配置摘要；专用 lifecycle 资源在 admin 权限下可读写结构化 `program/args`，但不返回解析后的敏感环境变量或 API key。执行响应返回当前状态、PID 是否归本应用所有、最近执行结果和 readiness 错误分类；start/stop/status 都必须是幂等操作，并带 `execution_id` 供 UI 轮询/关联 toast。

生命周期记录单独存储为 `provider_lifecycle_events` 或等价 append-only 表，不进入调用历史、route_activity 或 provider availability 热力图。记录字段包括 provider、动作、开始/结束时间、退出分类、状态转换、readiness 结果和脱敏错误摘要。

## Agent 配置能力契约

通用配置入口的注册、发现、scope、版本和审计规则以[配置能力注册表与 Agent API 计划](CONFIG-CAPABILITY-REGISTRY-PLAN.md)为上位契约；本节只补充 provider lifecycle 的高风险字段和领域动作。

“每增加一个人类可用配置入口，就自动拥有 Agent 可用 API”不能靠手写一组容易漂移的旁路端点实现。后续采用显式注册、自动派生的 capability registry：

```text
ConfigCapability {
  key: "providers.lifecycle.controllable",
  resource: "provider",
  value_schema: boolean,
  read_scope: "config:read",
  write_scope: "provider:lifecycle:configure",
  risk: "host_command",
  confirmation: "cooldown_5s",
  audit: true,
  version: 1
}
```

每个可见设置由同一份注册描述派生：

- UI 的标题、类型、默认值、禁用原因和确认要求。
- Agent 的发现目录与结构化读写 API。
- 校验器、权限 scope、风险等级、审计事件和版本迁移信息。
- 脱敏规则与错误码，避免 UI 接受而 Agent 拒绝，或反向产生隐式旁路。

首版建议提供：

```text
GET  /v1/admin/capabilities
GET  /v1/admin/configuration
PATCH /v1/admin/configuration
```

其中 `PATCH` 只接受注册过的 capability key 和类型化值；高风险动作仍使用领域端点，例如 lifecycle 的 `start/stop/status`，而不是让 Agent 传入任意路径或命令。注册表可以生成端点文档，但不能生成绕过领域校验的通用执行器。

权限至少分为：

| Scope | 能力 |
|---|---|
| `config:read` | 读取非敏感配置和脱敏运行状态 |
| `config:write` | 修改普通 provider/路由设置 |
| `provider:lifecycle:configure` | 修改 `controllable`、进程策略、命令结构和启动策略 |
| `provider:lifecycle:execute` | 执行已登记 provider 的 status/start/stop |
| `host_commands:enable` | 开启全局 `allow_host_commands`；仅管理员或本机用户确认 |
| `secrets:use` | 允许生命周期命令读取 API key 环境变量；不能读取明文返回 |

云端配置和执行都必须由管理员会话或等价管理员 Agent scope 完成；普通 caller token 永远不能写生命周期配置或执行主机命令。LAN 开放时远程 Agent 也遵守同一 scope，并在权限不足时返回结构化提示，而不是模糊的 `500`。

Agent/API 错误必须包含稳定 `code`、人类可读 `message`、`request_id`、必要时的 `execution_id` 和 `remediation`，例如：

```json
{
  "code": "host_commands_disabled",
  "message": "Host command execution is disabled for this instance.",
  "request_id": "req-…",
  "remediation": "An administrator must enable allow_host_commands after the safety confirmation."
}
```

权限失败、未注册 capability、provider 不可控、命令未配置、服务未就绪、超时和停止策略不允许都必须可区分。Agent 的每次配置修改和生命周期执行都写入审计事件，记录 actor、scope、capability、provider、结果和脱敏摘要。

## UI 交互

Provider 卡片保留紧凑摘要：“允许路由”“服务运行中/未运行/未知”。高级面板中提供：

1. 启动策略和命令编辑器（程序、参数、目录分栏）。
2. 环境变量预览，只显示变量名和敏感标记，不显示 key 值。
3. “检查状态”“启动服务”“停止服务”图标按钮；执行中禁用同 provider 的冲突操作。
4. toast 告知“命令已启动/服务已就绪/检查未知/停止失败”等结果，详情面板再显示 execution ID、耗时和脱敏输出摘要。
5. `startup_check` 在 daemon 监听后执行一次有界检查；明确返回 stopped 才启动，unknown、权限失败或超时不自动执行。

UI 文案必须使用“启动/停止服务”，不能把它写成 provider 的“启用/停用”；保留二值状态固定文案与视觉状态原则。

## 实施阶段

### Phase A：契约与安全模型

- [x] provider lifecycle 配置类型已加入 daemon config：`controllable`、`process_policy`、结构化程序/参数、工作目录、启动模式和有界超时；旧配置默认保持人工管理。
- [x] 全局 `allow_host_commands` 配置字段已加入，默认关闭；当前仅落盘/解析，不会执行命令。
- 冻结 `enabled`/`controllable`/`service_state`/`process_policy` 四维 schema、全局 `allow_host_commands`、退出码、状态机、敏感变量 allowlist 和本地/云/LAN/Android 权限边界。
- 冻结 capability registry、Agent scope、结构化错误码和审计字段，让 UI 与 Agent 共享同一配置契约。
- 做 Windows Job Object 与 Unix process group 最小 spike。
- 为 command parser、占位符解析、redaction 和状态归类写纯函数测试。

### Phase B：daemon 生命周期核心

- 新增 lifecycle 配置持久化/迁移，不改变旧 provider 快照语义。
- [x] 实现结构化 argv 的最小执行器、全局 host-command gate、controllable gate 和显式 stop/status/start 路由；不经过 shell。
- [x] 完成 readiness probe、超时、provider 互斥、进程所有权快照和独立 lifecycle 审计事件；未知归属不会误杀进程。
- [ ] 完成细粒度 Agent scope enforcement、Windows/Unix 进程组策略和重启恢复后的真实进程身份核验。

### Phase C：桌面 UI

- [x] Provider 高级面板、API key 注入确认模态框、toast 和状态徽标。
- Tauri/desktop local daemon 首先接入真实执行；浏览器 mock 使用 fake runner，不执行本机命令。Android local 的 UI 只在原生权限/存储门控完成后接入同一 API。

### Phase D：按需/启动检查

- [x] `startup_check` 在 daemon 监听后执行一次有界检查与明确 stopped 自动启动。
- [x] `on_demand` 在一次请求即将命中 provider 前启动并等待 readiness；超时按 provider failure 进入现有路由策略，不吞掉真实错误。

### Phase E：平台验收

- Windows：StartLux/OpenJev/Laya 各一条真实配置，覆盖启动、就绪、停止、端口冲突、权限拒绝和旧进程存在。
- Linux/macOS：process group、工作目录、shell 禁止规则和停止语义。
- Android local：覆盖 5 秒启用警告、进程策略、应用退出和原生 service 状态分离；Android LAN/cloud、云端和 LAN 远程管理员覆盖 scope、审计和失败提示，不把原生 service 状态误报为 lifecycle command 状态。

## 测试与验收矩阵

- 命令为空：现有 provider 行为完全不变。
- 未知占位符、shell 元字符、相对危险路径、超长输出、超时和非 UTF-8 输出均安全失败且不泄漏 key。
- API key 仅在明确引用并确认后注入；日志、toast、事件和 crash 产物中均无明文。
- 启动命令成功但 health 未就绪、状态命令 stopped/starting/unknown、停止命令失败分别呈现不同状态。
- 连续双击启动/停止、跨页面切换、daemon 重启和旧 PID 存在不产生重复进程或误杀。
- provider 路由 enabled 与 service running 的组合矩阵全部可读：允许/未允许 × 运行/未运行/未知。
- `controllable=false` 时生命周期配置不可执行，即使 `enabled=true`；云端 provider 默认以不可控服务呈现。
- `allow_host_commands=false` 时 UI、管理员 API 和 Agent API 均返回同一结构化禁用错误；开启动作必须经过 5 秒冷却确认。
- `persistent`、`session`、`external` 三种进程策略分别验证：退出 Jev-Switch 是否保留服务、是否只停止本应用拥有的进程、无所有权时是否禁用停止。
- `on_demand` 触发时只启动目标 provider；启动失败进入既有 `on_error` 策略并留下生命周期记录。
- [ ] Agent 使用不同 scope 访问 capability registry、配置写入和 lifecycle execute；当前领域错误已带稳定 code/message/remediation，scope enforcement 仍待实现。

## 不纳入首版

- 任意 shell/PowerShell 脚本编辑器。
- 自动下载模型、自动安装 Python/CUDA/Node 运行时。
- 通过端口扫描猜测并接管用户已有进程。
- Android LAN/cloud 上执行远程用户命令。
- 让 provider `enabled` 开关隐式触发启动/停止。
