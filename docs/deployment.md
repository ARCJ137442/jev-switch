# Jev-Switch 部署（Docker 云部署线 · 任务 #43）

> **作者**：MiMo（`mimo-v2.6-flash`，本次执行）· 如实披露
> **日期**：2026-09-23 · **关联**：`docs/12` §一/§二.3/§二.6/§二.7/§四线1 · `docs/contracts/05`（HTTP）· `docs/contracts/04`（密钥）
> **性质**：部署形态文档 + 契约部署偏差备案。**不修改任何 contracts/ 正文**。

---

## 一、快速开始（单命令）

```bash
# 1. 准备凭据（.env 已被 .gitignore 覆盖，密钥不进仓库）
cat > .env <<'EOF'
JEV_AUTH_TOKENS=tok-replace-me
JEV_ADMIN_PASSWORD=replace-me
# AI_GATEWAY_API_KEY=sk-…        # 可选：Vercel 上游
EOF

# 2. 起服务（构建 UI dist + Rust release → 单容器同源托管）
docker compose up -d

# 3. 探活（/health 双态皆放行，无需 token）
curl http://127.0.0.1:11435/health
```

- 持久数据：挂载整个 `./data` 目录，保留 SQLite 中的统一配置快照、入口、Token 和调用记录。`providers.toml` 首启自动播种并导入；之后人工修改 TOML 须通过控制台显式导入，不能覆盖数据库已确认的配置。用目录挂载而非单文件。
- 打开 `http://<主机>:11435/` 即控制台（daemon 同源托管 `ui/dist`，API 走相对路径，无跨源）。
- cloud 态凭据缺失时拒绝访问：公开调用需要调用 Token，管理操作需要管理员密码会话或 admin 角色 Token。示例中的环境变量用于首次启动；也可以先用管理员密码登录，再创建调用 Token。

运行镜像不调用 apt：TLS 根证书从同一 Debian 系列的 Rust 构建镜像复制，Docker `HEALTHCHECK` 调用 daemon 自带的 `--healthcheck` 子命令，核对 `/health` 的 `status`、产品身份和 API revision。这样运行层无需为探活额外下载 `curl` 或证书包。

Rust 构建会先按锁文件检查本机 BuildKit 依赖缓存；完整时直接离线构建，缺失时才联网获取（单次 fetch 上限 300 秒）。UI 的 npm 锁文件也由 `npm ci` 严格安装。缓存不能绕过 Rust/npm 锁文件校验，也不需要把个人 Cargo 凭据复制进镜像。网络受限时，排查构建阶段实际需要的 Cargo/npm 依赖源；运行层不再依赖 Debian CDN。

本地非容器跑法不变（README 快速开始三命令；默认 `mode = "local"`）。

Unix daemon 同时处理 SIGINT 和 SIGTERM。Docker/Compose 停机时先关闭监听，已受理请求继续完成；连接排空上限为 10 秒，超时后取消未完成连接。Compose 设置 `stop_grace_period: 15s`，给内核排空和退出留出时间；直接使用 `docker run` 时可设置 `--stop-timeout 15`。此行为不承诺撤销上游已经执行的计算或费用。

---

## 二、拓扑不区分原则（用户裁决 · docs/12 §一原文级）

> **`mode` 开关只管鉴权，完全不涉及上游拓扑。**「云端环境 / PC 环境」不做区分——内核访问 HTTP 地址一律**以内核所在的位置**来访问；云机同 LAN 有 Laya 就直连，没有就没有。

展开：

| 场景 | 行为（两态完全相同） |
|---|---|
| 内核与 Laya 同机 | `providers.toml` 写 `127.0.0.1:18765` → 自然直连 |
| 内核与 Laya 同 LAN | 写 LAN 地址如 `192.168.1.20:18765` → 自然直连 |
| 云机无 Laya | 配了就连、没配就不连——与 `mode` 无关 |
| 上游列表切换（laya ↔ vercel ↔ …） | 路由 DAG 职责，与 `mode` 无关 |

`mode = "cloud"` **不会**把 `127.0.0.1` 上游改写成别的地址，也**不会**发现/代理任何 LAN 服务——它是纯鉴权开关（+绑定地址）。

容器中的 `127.0.0.1` 指向容器自己；上游在另一容器时，可使用同一 Docker 网络内的服务名，例如 `http://laya:18765/v1/systemone`。不要直接把宿主机本地配置的回环地址复制到容器后期待它指向宿主机。

---

## 三、local / cloud 两态对照

| 维度 | `mode = "local"`（默认） | `mode = "cloud"` |
|---|---|---|
| 绑定 | `127.0.0.1:11435` | `0.0.0.0:11435`（容器内必需，见 §五偏差） |
| `/v1/systemone`、`/v1/models` | 仅限本机 loopback 调用，无需 Token | `Authorization: Bearer <调用 token>`；缺失/错值 → **401** |
| `/v1/admin/*` | 仅限本机 loopback 管理，无需 Token | 管理员密码换短时会话（2h），或使用 admin 角色调用 Token；readonly Token 无管理权限 |
| `/health` | 放行 | **放行**（探活需要，不设门） |
| 凭据来源 | 本机 loopback 无需凭据 | 调用 Token 由数据库管理；旧 TOML/env Token 一次性导入为 readonly，撤销后重启不复活。密码：`admin_password` 或优先的 `JEV_ADMIN_PASSWORD` |
| 上游拓扑 | 以内核所在位置访问 | **完全相同**（§二） |
| CORS | 5173 白名单 | **相同**（不扩公网 origin；同源托管下用不到 CORS） |
| 会话 | 不需要 | 进程内存，重启全废；改密码/重启 = 全体登出 |
| 适用 | 本机开发、LM Studio 式本地路由 | 公网/局域网中转站（sub2api 式） |

配置字段示例（`providers.toml`，Q4=b 明文哲学；详见 `rs/providers.example.toml` 注释）：

```toml
mode = "cloud"
auth_tokens = ["tok-…"]        # 或 [[auth_tokens]] 段；或 env JEV_AUTH_TOKENS
admin_password = "…"           # 或 env JEV_ADMIN_PASSWORD（env 优先）
```

`mode` 解析：文件 `mode` ← env `JEV_SWITCH_MODE` 覆盖；非法值（非 `local|cloud`）**拒绝启动**（带病不上线）。

---

## 四、云态鉴权风险块（部署前必读）

1. **公开调用需要有效 Token**：cloud 下未提供、停用或已撤销的调用 Token 均被拒绝。首次导入之后，以控制台 Token 管理进行轮换和撤销；只改 `.env` 并重启不会自动撤销数据库中已有的 Token。
2. **区分角色与凭据用途**：管理员密码会话用于管理；readonly 调用 Token 只允许公开调用、自身统计和自身事件；admin 调用 Token 还允许管理。管理员密码会话不充当公开调用 Token。首次部署须配置管理员密码，之后才能在管理界面创建受管理的调用凭据。
3. **会话是内存态**：不落盘、2 小时过期、重启全废。浏览器侧只存会话 token（`localStorage["jev_admin_session"]`），**密码从不落浏览器存储**。
4. **防火墙建议**：
   - `0.0.0.0:11435` 绑定 = 所有网卡可达；云主机请在安全组/防火墙**按需放行**（自用可只放行办公网 IP 或走 SSH 隧道）；
   - 暴露公网时**前置 HTTPS 反代**（nginx/caddy 终结 TLS）——Bearer 明文头在公网上必须加密传输；本仓 CORS/鉴权不管传输层加密；
   - 非必要不开 `0.0.0.0` 到无关网段；本地态（loopback）永远是更安全的默认。
5. **redact 兜底**：调用 token 与 admin 密码已纳入 daemon 的 known_keys 集——错误体/日志若意外拼进凭据值会被掩码（contracts/04 §2 红线 4 扩展面）；但**别依赖兜底**，上游侧日志仍须自己管住。
6. **探活无鉴权**：`/health` 两态放行，返回 `status/version/product/api_revision/build_revision` 供探针和桌面壳核对身份，不包含凭据。

---

## 五、契约偏差备案（部署形态，**不改 contracts 正文**）

| # | 契约条目 | 部署现实 | 处置 |
|---|---|---|---|
| 1 | `contracts/05 §1`：「Admin 仅绑 `127.0.0.1`」 | cloud 态必须绑 `0.0.0.0:11435`（容器内 loopback 绑定 = 端口映射不可达，云中转无从谈起） | **部署形态偏差**：cloud 态由**鉴权**（admin 会话门）而非绑定位承担防护；local 态维持 127.0.0.1 契约原状。契约文件不改，本表即备案 |
| 2 | `contracts/05 §1` 端点表无 login | cloud admin 需要「密码 → 会话」换发点 | 新增 `POST /v1/admin/login`（任务书 #43 A 项裁决字面）；现有契约端点的路径/形状**零改动** |
| 3 | `contracts/05 §5` CORS「Admin 默认不跨源；仅同主机 UI」 | dev 5173 白名单维持原样；cloud 默认同源反代托管 | **不扩公网 origin**（任务书 A 尾注：本期不给开关）；如需跨源访问自行在反代层配置——本期不支持运行时改 CORS |

---

## 六、静态托管选型（Dockerfile 二选一的答复）

**选：daemon 内 `tower_http::ServeDir` 挂 `ui/dist`（同源），不旁挂 nginx。**

理由：
1. **同源消解 CORS**：UI 与 API 同一 origin，浏览器零预检、零白名单问题；正式 UI 自动跟随托管源，包括自定义端口与 HTTPS 反代；
2. **单进程单端口**：镜像无第二个常驻进程，HEALTHCHECK/日志/重启语义简单；nginx 方案要多维护一个反代配置面；
3. 本项目 CORS 白名单语义（5173 dev + 同源 prod）已够用——nginx 在这里只搬运字节，不带来新能力。

代价与边界：TLS 终结、限流、访问日志切割等仍建议在云上**外层**反代做（§四.4）——那是传输层/边缘职责，与本选型不冲突。

dist 路径由 `JEV_UI_DIST` 指定（镜像内 `/app/ui/dist`；本地默认 `ui/dist`）。dist 缺失时 API 端点不受影响（fallback 只吃未匹配路由）。

---

## 七、给调用方 / UI 线的接口备注

**HTTP 头（两枚 token 分池）**：

| 场景 | 头 |
|---|---|
| cloud 调 `/v1/systemone`、`/v1/models` | `Authorization: Bearer <受管理的调用 token>`；旧配置/env Token 只在首次导入时成为 readonly 凭据 |
| cloud 调 `/v1/admin/*` | 管理员密码换取会话，或使用 admin 角色调用 Token；readonly Token 无管理权限（当前管理会话门返回 401） |
| 401 错误体 | `{"error":"…","upstream":null,"retryable":false}`（与契约 05 §3 同形，经 redact） |
| `/health`、`POST /v1/admin/login` | 不带任何鉴权头 |

**UI 存储键（已实现）**：
- 调用 Token：在“我的用量”中输入，仅保留于当前页面内存，刷新后须重新输入；旧 `localStorage["jev_token"]` 会清理。无 Token 时不附带调用鉴权头，local 回环调用仍可使用。
- admin 会话：`window.__JEV_ADMIN_SESSION__` 或 `localStorage["jev_admin_session"]`（api/admin.ts 读取；login 成功自动写入）
- 正式构建默认使用同源相对 API，不再用 11435 端口判断来源。Vite 开发环境默认连接 `http://127.0.0.1:11435`。确实需要分离 UI/API 时，页面加载前可设 `window.__JEV_BASE__ = "https://你的网关域名"`，并由部署者处理对应跨源配置。

**浏览器现状**：管理请求 401/403 接入登录提示，成功后重新拉取页面数据；调用者登录先清除旧管理员会话。只读身份仅保留仪表盘/演练场及自身数据。仪表盘配置读取失败显示权限/加载提示，不能误报为零个提供商或零条路由。

---

## 八、验证入口（本任务门禁）

| 层 | 命令/断言 | 状态 |
|---|---|---|
| Rust 全测 | `cargo test --manifest-path rs/Cargo.toml --workspace`（含 auth 中间件/login/config 双态专测） | 见提交记录 |
| UI 门禁 | `npm run lint --prefix ui` && `npm run build --prefix ui` | 见提交记录 |
| cloud 冒烟（本机 daemon） | `JEV_SWITCH_MODE=cloud` 起 daemon → 无 token 401 / 有 token 200 / login→admin 200 | 已实际验证；与容器验收分别记录 |
| compose 实跑 | 从当前 Dockerfile 构建；隔离 Compose 运行、受控上游调用、重建容器后恢复数据库状态 | 2026-09-25 已通过；详见[联调记录](verification/2026-09-24-entry-gateway-integration.md)；仍非已发布镜像 |
| local 回归 | `scripts/smoke.ps1`（免 token 现状） | 见提交记录 |

---

## 九、Tauri 桌面（Windows）· 任务 #44

> **作者**：MiMo（`mimo-v2.6-flash`，本次执行）· 如实披露 · **日期**：2026-09-23
> **裁决**：docs/12 §二.8「**Windows 优先**先出（GitHub Release 便携分发），mac/Linux 明确后置」+ §四-线3。
> **范围**：独立 Cargo 工程 `src-tauri/`（**不在** `rs/` workspace 内，互不干扰）。

### 9.1 形态（对标 LM Studio「双击开箱」）

| 项 | 行为 |
|---|---|
| 主窗口 | 当前默认 900×560 逻辑像素，常规最小 760×480；首次显示前按当前显示器工作区、DPI 与标题栏调整并居中，小工作区会同步下调最小尺寸。先载壳内等待页；核对 `/health` 的服务身份、接口修订与版本后切入 `http://127.0.0.1:11435`，UI 由 daemon 同源托管。见[响应式验收](responsiveness-check-2026-09-24.md)。 |
| Sidecar | 启动 spawn 打包进资源的 `jev-switch-daemon.exe`（= `rs` workspace 的 `jev-switch` bin，release 产物）；`JEV_SWITCH_CONFIG` 指向 `%APPDATA%\jev-switch\providers.toml`（**首启播种模板、已有不覆盖**，模板 = `src-tauri/src/default_providers.toml`，仅 `api_key_env` 示例值）；`JEV_UI_DIST` 指向打包的 `ui\dist`；**`JEV_SWITCH_MODE` 不设 = local** |
| 复用 | 若 11435 已有健康 daemon（手起/残留）→ 不重复 spawn，直接接入 |
| 单实例 | `tauri-plugin-single-instance`：二次双击聚焦已有窗口 |
| 托盘 | 左键显窗、右键菜单：显示主窗口 / 打开配置目录（explorer，自身单实例）/ 退出；**关窗 = 隐藏到托盘**，托盘退出才真退 |
| 退出 | `RunEvent::Exit` 统一收尸：`taskkill /T`（请求级）→ 2s 兜底 `kill`（Windows 控制台无 SIGTERM，此即优雅终止上限） |
| 图标 | `src-tauri/icons/icon.ico/.png` —— 黑底白 J 单色（System.Drawing 脚本生成 16–256 六档 ICO），无 emoji |

### 9.2 构建（本机已验，Windows / x86_64-msvc）

```powershell
# 一次生成 MSI、NSIS 与配套便携运行目录；不安装、不触发 UAC
.\scripts\build-windows-release.ps1

# 也可指定便携目录；目标必须不存在
.\scripts\build-windows-release.ps1 -PortableDestination "$env:TEMP\jev-switch-portable-build1"
```

前置：Rust（msvc）、Node、Tauri CLI（`cargo install tauri-cli`）、WebView2（NSIS 安装器自动引导下载）。脚本会先构建 daemon release 并更新 Tauri sidecar，再运行 Tauri UI hook 与 MSI/NSIS 构建，最后从同一组输入生成文件夹式便携运行时。`src-tauri/binaries/*.exe`、`src-tauri/target/`、`src-tauri/gen/` 已 gitignore。

产物（2026-09-23 实测）：

| 产物 | 路径 | 大小 |
|---|---|---|
| NSIS 安装包 | `src-tauri/target/release/bundle/nsis/jev-switch_0.1.0_x64-setup.exe` | 3,424,907 B（≈3.3 MB） |
| MSI 安装包 | `src-tauri/target/release/bundle/msi/jev-switch_0.1.0_x64_en-US.msi` | 4,927,488 B（≈4.7 MB） |
| 历史便携布局（2026-09-23） | `src-tauri/target/release/`（`jev-switch.exe` + `jev-switch-daemon.exe` + `ui\dist\` 同目录） | — |

安装布局（NSIS/MSI 一致，实测解包确认）：`$INSTDIR\` 平铺 `jev-switch.exe`、`jev-switch-daemon.exe`、`ui\dist\…`——壳的 sidecar/UI 路径解析器按此多候选探测（打包 + dev 双布局）。

2026-09-26 起，便携验收目录由 `scripts/build-windows-release.ps1` 从同次 MSI/NSIS 构建输入组装，具体路径与 SHA-256 写在每份 `build-manifest.json`；发布流水线另将目录压为 Windows x64 ZIP。最新执行与哈希见[总计划](design/ENDPOINT-GATEWAY-ALIGNMENT-PLAN.md)及[发版手册](RELEASE.md)。

### 9.3 实测证据（双击开箱门禁，2026-09-23）

| 断言 | 证据 |
|---|---|
| 窗口起来 | 进程 `jev-switch.exe`（pid 56544）`MainWindowTitle=Jev-Switch`，窗口截图见 `src-tauri` 任务归档（截图在 `target/shot.png`，不进 git） |
| webview 加载 UI | 子进程 `msedgewebview2.exe`（EBWebView 数据目录 `io.github.arcj137442.jevswitch`）；截图可见控制台 Providers 页 + `DAEMON OK` + 页脚 `ENDPOINT 127.0.0.1:11435` —— 即 webview 已从等待页切入 `http://127.0.0.1:11435` |
| sidecar 活 | 子进程 `jev-switch-daemon.exe`（pid 71552，`Win32_Process.ParentProcessId=56544`）LISTEN `127.0.0.1:11435` |
| health | `GET /health` → `{"status":"ok","version":"0.1.0"}`；`GET /` → 200（ServeDir 托管 `ui/dist`，title `Jev-Switch — Multi-model decision router`） |
| 配置首播 | `%APPDATA%\jev-switch\providers.toml` 落地 5173 B（已有不覆盖——二次启动不重写 mtime 语义在 `seed_config` 判断 `!exists()`） |
| 单实例 | 第二次双击后壳进程数仍 = 1 |

### 9.4 施工备注（偏差备案，不动 contracts）

1. **`beforeBuildCommand` = `npm run build --prefix ui`**（任务书字面 `../ui` 不可用）：tauri-cli 的 hook cwd 是 **`src-tauri` 的父目录（仓库根）**，`--prefix ../ui` 会解析成 `Jev\ui` 而 ENOENT；`--prefix ui` 语义等价（保证 dist 新鲜），已实测。
2. **sidecar 命名 `jev-switch-daemon` 而非 `jev-switch`**：externalBin 落地时剥 target-triple，若与壳主二进制同名 `jev-switch.exe` → WiX **ICE30**（两组件装同一文件名）→ MSI light 失败。改名后 nsis+msi 双绿。
3. 打包时 `Failed to add bundler type … __TAURI_BUNDLE_TYPE` **warn**：无 updater 插件场景的已知无害告警，不影响安装包。
4. 打开配置目录 = `explorer <dir>`（explorer 自身单实例，不再造轮子）。
5. 等待页不跑跨源 fetch：就绪轮询放在壳 Rust 侧（纯 std TCP 探 `/health`），核对服务身份及版本后再切入；不能把任意 HTTP 200 当成兼容内核。

### 9.5 后置（❄️ 不做，docs/12 §三冻结清单）

- Tauri **macOS / Linux** 打包与签名（Windows 首发裁决）；
- 托盘「退出」路径的 UI 自动化回归（本次人工验收级：收尸逻辑在 `RunEvent::Exit` 单点，代码审阅覆盖）。

---

## 十、mode 热切换与 listen 热 Rebind（不重启）+ 重启粒度（小网关）

> **作者**：MiMo（`mimo-v2.6-flash`，Mode-Agent 本次执行）· 如实披露
> **日期**：2026-09-23 · **裁决**：用户「服务器上重启是大阻碍」→ 方案一「改地址不重启」+ 任务级最小重启 + 激活原子设密。
> **性质**：在 §三/§五基础上的行为**增补与局部变更**（§三表未改动；冲突以本节为准）。含 contracts/05 §1 端点表新增备案（同 §五 login 处理方式）。

### 10.1 新旧行为对照（bind / mode）

| 维度 | 旧行为（#43 初版） | 新行为（本节落地） |
|---|---|---|
| bind 来源 | **隐含在 mode 里**（local→`127.0.0.1:11435`、cloud→`0.0.0.0:11435`，不可另配） | **独立配置**：文件 `bind = "ip:port"` ← env `JEV_BIND` 覆盖；未配置 → **成对默认随 mode**（地址同旧，来源显式化） |
| 换监听地址 | 只能改配置 + **重启进程** | **`PUT /v1/admin/listen` 热 Rebind**（try-bind → 优雅退场 → spawn；零进程重启、零内核重建） |
| mode 切换 | 改配置 + 重启 | **`PUT /v1/admin/mode` 运行时热切**：写鉴权 `RwLock` → 后续请求**立即生效**（在途请求跑完旧策略）+ 写回 toml；**非显式 bind 时联动 Rebind 到成对默认** |
| local 态防护 | 靠 loopback 绑定（内核级） | 不变（非显式绑定仍 `127.0.0.1` 内核级关）；**显式 bind=0.0.0.0 时**追加应用层 peer 兜底：非 loopback → **403**（`local mode: loopback only`，三键错误体） |
| admin 密码 | 仅启动期读 env/toml | **运行时热更**：`PUT /v1/admin/password`；mode 激活可同请求携带首个密码（见 10.6） |
| 运维可观测 | 只有 `/health` | **`GET /v1/admin/status`** 首页仪表盘自检（见 10.7） |

### 10.2 语义矩阵（两 mode × peer；鉴权中间件每请求读锁取 mode）

| 端点域 | local + loopback peer | local + 远程 peer | cloud + loopback peer | cloud + 远程 peer |
|---|---|---|---|---|
| `/v1/systemone`、`/v1/models` | 放行（零鉴权，现状） | **403** | Bearer 调用 token，缺失/错值 401 | 同左（401） |
| `/v1/admin/*`（providers/routes/mode/listen/status/probe） | 放行（零鉴权，现状） | **403** | admin 会话，缺失/无效 401 | 同左（401） |
| `POST /v1/admin/login` | 放行（密码错仍 401） | 放行（门外端点） | 放行（密码错/未配 401） | 放行（门外端点） |
| `PUT /v1/admin/password` | 放行 | **403** | **会话 或 loopback 皆可**（忘密恢复） | 需会话，无 → **401** |
| `/health`、静态 UI | 放行 | 放行（探活需要；静态 UI 不设门） | 放行 | 放行 |

- peer 取 `ConnectInfo<SocketAddr>`；**peer 缺失（进程内 oneshot/单测无 TCP 对端）按 loopback 信任** —— 真实 TCP 必有 ConnectInfo，None 仅测试路径。
- 「local + 远程 → 403」只在**显式 bind 让 LAN 可达**时才可能被触发（非显式默认绑 `127.0.0.1`，连接层直接 refused）——这是 peer 校验作为**显式绑定残留场景兜底**的定位。

### 10.3 bind 默认变更与 LAN-403 说明（显式绑定代价）

- **默认（未配 bind）**：local → `127.0.0.1:11435`、cloud → `0.0.0.0:11435`（成对；与 §三表地址一致，来源从「mode 驱动」改为「独立键缺省」——README/脚本地址不变）。
- **显式 `bind`（文件或 `JEV_BIND`）**：恒绑该值，mode 翻转**不动监听**（响应 `rebind.skipped = "explicit bind"`）。若显式值是 `0.0.0.0:…` 而 mode=local：LAN 内 **TCP 可达**，但应用层 peer 校验对 `/v1` 一律 **403**（10.2 矩阵）。此代价已接受；如需内核级关闭，**不要显式配 0.0.0.0**（用成对默认），或自行加防火墙。
- 非法 `bind`/`JEV_BIND`/`JEV_SWITCH_MODE` → **启动硬拒**（带病不上线）。

### 10.4 热切端点（curl 示例）

```bash
# ① mode 热切：local → cloud（非显式 bind 时自动 Rebind 到 0.0.0.0:11435）
curl -X PUT http://127.0.0.1:11435/v1/admin/mode \
  -H 'content-type: application/json' \
  -d '{"mode":"cloud"}'
# → {"mode":"cloud","persisted":true,"env_override_active":false,
#    "rebind":{"from":"127.0.0.1:11435","to":"0.0.0.0:11435","ok":true}}
# cloud 态发起翻转必须带 admin 会话（防匿名拆锁）：
#   -H "Authorization: Bearer <login 换取的会话 token>"

# 首次激活 cloud 且从未配过密码 → 必须同请求带第一个密码（否则 400 指引）：
curl -X PUT http://127.0.0.1:11435/v1/admin/mode \
  -H 'content-type: application/json' \
  -d '{"mode":"cloud","admin_password":"…"}'

# ② 监听热 Rebind（显式化；写回 toml bind 键，此后 mode 翻转不再动它）
curl -X PUT http://127.0.0.1:11435/v1/admin/listen \
  -H 'content-type: application/json' \
  -d '{"addr":"127.0.0.1:2222"}'
# → {"addr":"127.0.0.1:2222","rebound":true}
# 恢复成对默认（按当前 mode 自动选地址；从 toml 移除 bind 键）
curl -X PUT http://127.0.0.1:11435/v1/admin/listen \
  -H 'content-type: application/json' -d '{"addr":"auto"}'
# 查询当前实际监听：
curl http://127.0.0.1:11435/v1/admin/listen
# try-bind 失败（目标端口被占）→ 500 三键错误体，旧监听一字未动、不写文件。

# ③ admin 密码热更（轮换 = 全体会话作废，代际 +1）
curl -X PUT http://127.0.0.1:11435/v1/admin/password \
  -H 'content-type: application/json' \
  -d '{"password":"…"}'
# cloud 鉴权：有效会话（任意 peer）**或** loopback（无会话）——见 10.2 矩阵
```

**响应字段（给 UI 的 mode 切换钮）**：

| 端点 | 成功体 | 鉴权 |
|---|---|---|
| `PUT /v1/admin/mode` | `{mode, persisted, env_override_active, rebind:{from,to,ok[,reason]} \| {skipped:"explicit bind"\|"no listener"}}` | admin 门内：cloud=会话；local=loopback |
| `PUT /v1/admin/listen` | `{addr, rebound:true, reason?}`；失败 400/500 三键错误体 | 同上 |
| `GET /v1/admin/listen` | `{addr}` | 同上 |
| `PUT /v1/admin/password` | `{updated:true, env_override_active}`（**永不回显密码**） | 门外 in-handler：见 10.2 |

**Rebind 时序（try-bind → 停 accept 确认 → 切换；旧连接独立排空）**：
1. 地址与当前**不重叠**（不同端口/IP 不互含）：先 **try-bind 新地址**。失败直接返回错误，旧监听保持原样；成功后停止旧 listener、确认套接字释放，再启动新服务并更新共享 `bound` 地址，随即回复切换请求。
2. **同端口重叠例外**（如成对默认 local⇄cloud 同为 `:11435`）：新地址先试绑失败且与旧地址重叠时，先关闭旧 listener 并确认释放，再试绑新地址。失败则恢复旧地址后报错；若旧地址恰被其他进程抢占，返回明确的 `restore failed`，不能保证这种竞争下服务仍可用。
3. 两种路径都让已接受的连接独立排空。旧连接上的切换请求可正常收到响应，在途上游请求继续完成；超过 `DRAIN_TIMEOUT=10s` 的剩余连接（包括无限 SSE）会被取消并释放。**10 秒是连接清理上限，不是切换接口必须等待的时间**，否则切换请求等待自己结束会形成互相等待。

2026-09-25 的隔离实测：Windows 换端口并恢复约 32ms/30ms；Docker 同端口 cloud→local / local→cloud 为 5.2ms/4.8ms。它们是本次环境的观测值，不是响应时间保证。真实 TCP 测试另覆盖旧端口关闭、慢请求完成、重叠绑定失败恢复和 SSE 超时取消，证据见 [联调记录](verification/2026-09-24-entry-gateway-integration.md)。

启动日志中的 `legacy_config_tokens` 仅是配置文件/环境中旧式调用 Token 的数量；真正托管调用权限以持久存储为准。紧随其后的 `managed call tokens restored` 给出启用数/总数，不能因前者为零就认定所有调用都会失败。日志只记录数量，不输出凭据。

**重启粒度声明**：本次全部热切均在**任务级**（tokio task + socket 替换；内核 Router/auth/handlers 的 `Arc` 共享、**永不因换地址而亡**；进程级零重启）。
进程级「独立小网关」方案**本期不实现**——可选演进：将 listen 层拆独立进程做故障隔离（daemon 崩不影响网关 accept 队列等场景），需要时再立项。

### 10.5 env 覆盖警示（`env_override_active`）

- `PUT /v1/admin/mode` **总是**把 mode 写回 toml（Q5 文件真值）；若 env `JEV_SWITCH_MODE` 非空 → 响应 `env_override_active: true`——**下次启动 env 会覆盖文件值**（本次运行态已切，重启后回到 env 定的 mode）。
- 密码写入（mode 携带 / password 端点）同理：env `JEV_ADMIN_PASSWORD` 非空 → 警示 true（下次启动覆盖回 env 值）。UI 见到 true 应提示用户「改 env 或移除 env 才能持久」。
- 优先级恒为 **env（非空）> 文件**（与 §三表一致）；`status.env_override_active` 反映的是 **mode** 的 env 活跃位。

### 10.6 部署矩阵（首个管理员密码 · 忘密恢复 · 轮换）

| 场景 | 做法 | 行为 |
|---|---|---|
| 服务器 compose | `.env` 注入 `JEV_ADMIN_PASSWORD` + `JEV_AUTH_TOKENS` + `JEV_SWITCH_MODE=cloud`（§一 快速开始） | env 优先于文件；重启后仍是 env 值 |
| 桌面 / 本地 | toml 写 `admin_password` / `auth_tokens`（Q4=b 明文哲学，0600 文件） | 不设 env 则文件即真值 |
| 忘配密码但想开 cloud | `PUT /mode {"mode":"cloud","admin_password":"…"}` **一发激活** | 无密码且不带 → **400** 指引文案（fail-closed 不破）；带了 → 写文件+立即生效+会话代际作废+激活 |
| **忘密恢复（cloud 运行中）** | 本机（loopback）一行 curl `PUT /v1/admin/password`，**无需任何会话** | Q4 本地信任=root 等价；零重启修复部署矩阵最后一个洞。**远程**非 loopback 无会话 → 401（必须 SSH/本机） |
| 密码轮换 | 任一路径（mode 携带 / password 端点）都走**同一内部入口**（`set_admin_password`） | 写 toml + 运行时生效 + **全部旧会话作废**（代际 +1）+ env 警示 |
| 旧会话语义 | 改密后旧 session token 立即 401 | 浏览器需重新 login（自动触发） |
| remote 翻转顺序 | local 态下**远程无法**调 `/v1/admin/mode`（403 天然挡住）——**先在本机翻到 cloud 才能远程管理** | 这是正确语义非 bug；且激活那一发就带上第一个密码（10.6 闭环），之后远程持会话接管 |

### 10.7 `GET /v1/admin/status` —— 首页仪表盘 / 运维一眼自检

```bash
curl http://127.0.0.1:11435/v1/admin/status
# cloud 态带会话：-H "Authorization: Bearer <会话>"
# → {"mode":"cloud","bind":"0.0.0.0:11435","bind_explicit":false,
#    "env_override_active":false,"password_set":true,
#    "version":"0.1.0","uptime_s":120}
```

| 字段 | 含义 |
|---|---|
| `mode` | 当前运行模式（Rebind/热切后即时） |
| `bind` | **实际当前监听地址**（Rebind 后实时反映） |
| `bind_explicit` | 是否显式配置 bind（true = mode 翻转不改监听） |
| `env_override_active` | `JEV_SWITCH_MODE` 环境变量活跃（文件会被覆盖） |
| `password_set` | 管理密码是否已配置——**绝不返回任何密码值** |
| `version` / `uptime_s` | 进程版本 / 启动至今秒数 |

**运维用法**：一条 curl 定位「当前什么模式、监听在哪、是不是显式钉死、env 在不在覆盖、密码设没设、进程活了多久」——7 键形状冻结（单测守形状）；鉴权同 admin 集合（cloud 会话 / local loopback，login 除外规则不变），未登录 → 401 三键。

**新增端点备案**（contracts/05 §1 端点表之外，同 §五 login 处理方式——契约文件不改，本节即备案）：
`PUT /v1/admin/mode`、`PUT|GET /v1/admin/listen`、`PUT /v1/admin/password`、`GET /v1/admin/status`。错误体一律 §3 三键形状（listen 失败 = 400 解析 / 500 try-bind 失败 / 503 无 supervisor）。
