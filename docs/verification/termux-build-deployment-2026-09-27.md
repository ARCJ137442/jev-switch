# Termux Rust Workspace 与 UI 构建核验（2026-09-27）

## 范围与结论

核验对象为 Jev-Switch `main` 的 HEAD `8b626c2880091b04e66cb9b274a6c89318c9a04b`（提交时间 `2026-09-27T05:18:29+08:00`），在 Termux / Android arm64 环境检查 Rust workspace 的联网与离线门禁、UI 门禁及运行部署边界。本文记录当前工作树实测，不把历史 CI、其他平台或本机生成物的结果外推到新 checkout。

当前 Rust 与 UI 源码门禁均在本机通过。首次离线 Rust 尝试因本地索引缺少锁定的 `futures 0.3.34` 失败；联网门禁补齐缓存后，联网 workspace 测试和 `ts-rs` feature 测试均通过。UI 的 `node_modules` 在本次检查时存在，但该目录被 Git 忽略，不包含在 checkout 中；本次随后执行 `npm ci` 并复跑 UI 门禁，新环境仍应依据 `ui/package-lock.json` 执行该命令。本次没有验证 Tauri 桌面包、Docker 构建或真实 Jev 上游请求。

## 环境与版本同步

| 项目 | 实测值 |
|---|---|
| 日期 | 2026-09-27 UTC |
| 平台 | Termux，Android 15 API 环境；Linux `aarch64`，内核标识 `5.15.167-android13` |
| Rust | `rustc 1.93.1`，host `aarch64-linux-android`；`cargo 1.93.1`；未安装 `rustup` |
| JavaScript | Node.js `v25.3.0`；npm `11.10.0` |
| Docker | 未安装 |
| 仓库 HEAD | `8b626c2880091b04e66cb9b274a6c89318c9a04b` |

Rust workspace 在 `rs/`；Tauri 壳是 `src-tauri/` 下的独立 Cargo 工程。CI 的默认后端/前端门禁见 [ci.yml](../../.github/workflows/ci.yml)，发布版本同步检查位于 [release.yml](../../.github/workflows/release.yml)。当前四处应用版本字段均为 `0.1.0`：`ui/package.json`、`src-tauri/tauri.conf.json`、`rs/Cargo.toml` 与 `src-tauri/Cargo.toml`。锁文件分别为 `ui/package-lock.json`、`rs/Cargo.lock` 和 `src-tauri/Cargo.lock`。版本字段与 HEAD 对齐；这不是一次版本升级或发布验收。

Termux 环境未安装 `rustup`，因此本机工具链不是通过 rustup channel 解析的 `stable` 安装；本次记录精确版本号，不宣称等同于 GitHub Actions 的 Ubuntu stable 工具链。

## Rust workspace 门禁

仓库 README / CI 以锁文件为约束；本次先运行离线门禁，观察到缓存缺口，再允许 Cargo 联网补齐依赖并运行正式门禁。不要把“补齐后可运行”倒推成“空缓存离线可运行”。

| 命令 | 结果 |
|---|---|
| `cargo test --manifest-path rs/Cargo.toml --workspace --locked --offline`（首次） | 退出码 101；本地 crates.io 索引没有锁定的 `futures 0.3.34`，不是源码测试失败 |
| `cargo test --manifest-path rs/Cargo.toml --workspace --locked` | 退出码 0；默认 workspace 测试全通过（218 个非 doc 测试） |
| `cargo test --manifest-path rs/Cargo.toml --workspace --features ts-rs --locked` | 退出码 0；`ts-rs` feature 测试全通过（275 个非 doc 测试）。输出若干 serde alias 解析 warning，但无测试失败 |
| `npm ci --prefix ui` | 退出码 0；安装 134 packages。npm 报告 2 个 audit vulnerabilities（1 moderate、1 high），本次未自动升级依赖 |
| `git diff --exit-code -- ui/src/generated rs/crates/jev-protocol/bindings` | 退出码 0；生成类型无 tracked diff |
| `cargo fmt --manifest-path rs/Cargo.toml --all -- --check` | 退出码 0 |
| `cargo metadata --manifest-path rs/Cargo.toml --locked --offline --no-deps --format-version 1` | 退出码 0 |

本机 Cargo registry 和工作区 `rs/target` 已有构建缓存；缓存大小会随工具链与历史构建变化，因此不作为可移植门槛。首次离线缺包的结果已经说明空缓存不可假定可离线完成。带 `--locked` 时锁文件不会由上述测试门禁更新。本次联网门禁使用正常 Cargo 网络行为，但不是对每个 crate 下载源、代理、全新缓存或受限网络的独立连通性保证。

### Rust 门禁失败时的区分方式

- `--offline` 失败并提示 crate/索引缺失，首先表示本地缓存不完整；在允许联网的环境预取依赖后再重跑。联网补齐后本次没有把完整 workspace 离线命令作为独立成功证据，因此不要把离线门禁写成已通过。
- 默认联网命令失败于 DNS、TLS、代理、索引或 crate 下载，不能据此断言源码测试失败；应保留 Cargo 原始错误，先解决依赖获取，再检查测试结果。
- `--locked` 失败提示 lockfile 需要更新，表示 manifest 与锁文件不一致；不要通过移除 `--locked` 掩盖 CI 漂移，应按项目流程更新并审阅 `rs/Cargo.lock`。
- Android/Termux 通过只覆盖此 host 的源码编译与测试，不证明 Linux glibc、Windows、Tauri 桌面或容器镜像构建通过。

## UI 依赖与门禁

本次开始时 `ui/node_modules` **存在**，大小约 125 MiB；`npm ls --prefix ui --depth=0` 成功，`tsc` 与 `vite` 可执行。该依赖目录由 `.gitignore` 排除，HEAD 没有跟踪 `node_modules`；因此“当前 Termux 缺少 node_modules”不符合本次实测，而全新 clone/archive 确实不会包含它。随后已实际执行 `npm ci --prefix ui` 并复跑门禁；`ui/package-lock.json` 是提交内的依赖真值，新环境应运行该命令安装。

| 命令 | 结果 |
|---|---|
| `npm test --prefix ui` | 退出码 0；21 项通过，0 失败 |
| `npm run lint --prefix ui` | 退出码 0；运行 `tsc --noEmit` |
| `npm run build --prefix ui` | 退出码 0；`tsc --noEmit` 与 Vite 5.4.21 生产构建通过，转换 1956 modules |

若新 checkout 没有 `ui/node_modules`，直接运行上述 npm 脚本会因依赖二进制不存在而失败；先执行锁文件驱动的 `npm ci --prefix ui`。不要把本机忽略的 `ui/dist/` 作为源码或版本同步输入；它由成功的 UI build 生成，并在部署/打包前按当前 HEAD 重建。

## 版本、运行与部署建议

1. 先确认 checkout 的 HEAD/工作树及四处 `0.1.0` 版本字段与锁文件；不要将这个核验记录视为当前发布或改变发布版本的授权。
2. 在具备网络的 Termux 环境先执行 `cargo test --manifest-path rs/Cargo.toml --workspace --locked`，让缺失的 crates 按 `Cargo.lock` 下载；再按需重跑 `--offline`，但把离线成功与否作为当前缓存状态的单独证据。需要验证前端时，执行 `npm ci --prefix ui`，再按 CI 顺序运行 lint、test、build。
3. 日常本机 daemon 可按仓库示例配置运行；应明确配置路径、监听模式和端口，并检查 `/health`。Termux 上的 `127.0.0.1` 是 Android 设备本机回环，不自动意味着其他设备或容器可访问；若要对外监听，须单独评估认证、绑定地址与网络暴露。
4. 把源码测试与服务部署分开：本机通过 `cargo test` 不等于 daemon 已部署、进程已重启或新 `ui/dist` 已被正在运行的服务加载。部署前重建目标二进制与 UI，部署后核对进程身份、版本/`build_revision`、监听地址、`/health` 和实际服务资源。
5. Termux 未安装 Docker，故容器建议仍按仓库的 Dockerfile/Compose 文档在支持 Docker 的主机执行；不要从本次 Android Rust 测试推断 Docker 镜像可构建或可运行。

## 部署烟测与 CLI/TUI 方向

使用当前 workspace 已构建的 `rs/target/debug/jev-switch`（`file` 报告为 Android/aarch64 ELF），配合临时空 provider 配置、临时数据目录和当前 `ui/dist` 完成了本地启动烟测：

- `GET /health` 返回 `status=ok`、`version=0.1.0`、`product=jev-switch`、`api_revision=1`、`build_revision=null`；
- `GET /v1/models` 返回 HTTP 200，空配置下 `data=[]`、`upstreams=[]`；
- `GET /` 返回 HTTP 200，daemon 能同源托管当前 UI；
- 进程退出后未保留监听端口；临时配置和数据库不属于仓库。

该烟测只证明 daemon、SQLite 初始化、健康端点、空模型表和静态资源托管在本机成立，不证明真实上游调用或后台服务管理。首次脚本实验还暴露了 Termux wrapper linker warning 污染命令替换结果的风险；维护脚本应避免把带 warning 的 stdout 直接用于路径变量，并对路径使用显式绝对值。相关 warning 不应写入配置文件名或工作区。

CLI/TUI 是后续产品方向，不是当前 `v0.1.0` 已实现能力；本机没有构建或试用 CLI/TUI。按当前 daemon 源码，建议分两步推进：

1. 先做无状态 CLI 薄客户端，可从 `status`、`models`、`invoke`、`routes`、`events` 开始：分别复用 `GET /health` 与 `GET /v1/admin/status`、`GET /v1/models`、`POST /v1/systemone`、`GET/PUT /v1/admin/routes`、`GET /v1/admin/events`。事件页使用 `since`/`limit` 游标；输出支持 JSON 与便于终端阅读的表格。若命令最终命名为 `logs`，应明确它展示的是 daemon 持久化的调用活动/路由元数据，不是任意进程 stdout/stderr；后者目前没有通用日志读取 API。
2. 再做 TUI，复用同一 HTTP client 与协议 DTO，展示入口和 DAG、调用状态、route trace、usage、latency、错误及历史。实时更新可评估 `GET /v1/admin/events/stream`，恢复/翻页使用 `GET /v1/admin/events` 的持久 ID 游标；caller-scoped 场景另有 `/v1/events/my` 与 `/v1/events/my/stream`。不要在 TUI 中复制路由、SQLite 或鉴权实现。

Termux 首期宜支持本机 daemon 与 loopback。远程/cloud 模式须遵循 daemon 现有调用 token/admin session 鉴权；不要把 token 放入命令行参数、shell history 或日志，也不要让 CLI/TUI 接管 provider key。CLI/TUI 可作为独立 Rust 客户端 crate/二进制接入 workspace，不把终端 UI 状态放进 `jev-core`。验收至少覆盖 local/cloud 鉴权、断线与重连、迟到响应、取消、事件游标续读、窄终端/resize、非 TTY 与无色输出、敏感字段脱敏。命令契约、配置存储和 TUI 框架仍待产品决策。

## 未验证边界

- `npm ci` 已在当前 checkout 执行并成功；尚未在完全空缓存/无网络环境重现 npm 安装，因此不能宣称离线首次安装可行。
- 未验证 `src-tauri/` 桌面壳构建/测试。项目当前发版工作流面向 Windows，Termux Android host 不等同于 Tauri 发布 target；本文不宣称 Android 桌面打包支持。
- 未验证 Docker daemon、Dockerfile、Compose、镜像体积或容器生命周期；此环境无 Docker CLI/daemon。
- 未测试生产二进制的安装、后台服务管理、自启动、Android 电池优化、设备重启恢复、局域网访问、反向代理或 TLS。
- 未调用 Vercel 或 Laya 上游；没有 API 凭据读取、输出或写入本报告，也未验证真实 API 费用、延迟与模型质量。
- 多条命令的 stderr 都出现 Android linker 环境警告：`failed to find generated linker configuration from "/linkerconfig/ld.config.txt"`。本次相关命令均以相应记录的 exit code 完成；这只说明该警告未阻止本次门禁，不判定 Android 系统配置原因或所有设备上的影响。一次烟测脚本曾因此生成异常临时目录，已按精确路径清理；维护脚本不得将该类 warning 拼入文件名。
- 早先一次工具调用输出曾短暂显示一个由该 linker 警告命名的异常未跟踪目录；随后复核的当前工作树未再发现它，不能据此判断为仓库产物。它不属于 Jev-Switch 源码，本次未触碰、未纳入本报告。核验期间 `node_modules` 与 `dist` 是忽略目录，`rs/target` 为构建目录，均不构成源码变更。

## 可复核命令

以下命令不要求供应商/API 密钥，均从仓库根目录运行：

```bash
git rev-parse HEAD
rustc -Vv
cargo -V
node --version
npm --version
cargo test --manifest-path rs/Cargo.toml --workspace --locked
npm ci --prefix ui
cargo test --manifest-path rs/Cargo.toml --workspace --features ts-rs --locked
git diff --exit-code -- ui/src/generated rs/crates/jev-protocol/bindings
cargo fmt --manifest-path rs/Cargo.toml --all -- --check
npm test --prefix ui
npm run lint --prefix ui
npm run build --prefix ui
```

本机完整命令输出未作为仓库文件提交；只记录了可复核的命令、退出码和不含凭据的摘要。

---

GPT-5.6 Sol ultra（OpenAI Codex），Termux 源码/构建核验，2026-09-27
