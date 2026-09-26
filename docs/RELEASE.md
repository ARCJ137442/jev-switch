# 发版手册

**流水线**：`.github/workflows/release.yml`（测试门禁复用 `ci.yml`）

---

## 发一个版本

当前仓库四个发布版本字段及 `ui/package-lock.json` 均为 `0.1.0`。本次按这个已配置版本发稳定版 `v0.1.0`：远端已有的 `v0.1.0-mvp` 是较早的 MVP tag/Release，`v0.5.0-stable` 是历史代码 tag，均不等于包内 SemVer；精确的 `v0.1.0` tag 尚不存在。`0.6.0` 仍只是下列语法示例。版本门禁以 tag（`vX.Y.Z`）为准，并要求以下四处与 tag 去掉 `v` 后一致：

- `ui/package.json` 的 `version`
- `src-tauri/tauri.conf.json` 的 `version`
- `rs/Cargo.toml` 的 `[workspace.package].version`
- `src-tauri/Cargo.toml` 的 `[package].version`

Rust 与 npm 锁文件也要跟随版本/依赖变动更新：`rs/Cargo.lock`、`src-tauri/Cargo.lock`、`ui/package-lock.json`。Cargo 锁文件由普通 `cargo check` 更新；npm 锁文件可用 `npm install --package-lock-only --prefix ui` 更新。随后用下列锁文件严格模式确认没有漂移：

```bash
# 以 0.6.0 为例，手动把上面四处版本字段统一改成该版本
cargo check --manifest-path rs/Cargo.toml --workspace
cargo check --manifest-path src-tauri/Cargo.toml
npm install --package-lock-only --prefix ui

# 与 CI 相同的锁文件检查
cargo test --manifest-path rs/Cargo.toml --workspace --locked
cargo test --manifest-path rs/Cargo.toml --workspace --features ts-rs --locked
cargo test --manifest-path src-tauri/Cargo.toml --locked
npm ci --prefix ui
npm run lint --prefix ui
npm test --prefix ui
npm run build --prefix ui

# 检查所有变更（含未跟踪的新文件），只暂存本次要发布的文件
git status --short --untracked-files=all
# 根据上面的清单，把本次要发布的每个路径显式写入 git add 命令
git diff --cached --check
git diff --cached --stat
git diff --cached

git commit -m "chore: bump 0.6.0"
git tag v0.6.0
git push origin main v0.6.0    # 推 tag 触发发版
```

不要用 `git commit -am` 代替显式暂存：它只包含已跟踪文件，新增的迁移、测试、`ui/src/generated` 类型、源码或文档不会进入 tag。Workflow 只构建 tag 对应的提交，工作树里未提交的修改不会参与发版。CI 的 ts-rs 门禁会同时检查生成目录中的已跟踪差异和未跟踪文件。

CI 与 release Windows job 都用 `npm ci`，Rust workspace 测试用 `--locked`；release Windows job 的 daemon build 和壳测试也用 `--locked`。Tauri bundle 命令本身没有显式传 `--locked`，因此前面的锁文件检查必须通过。

Actions 页查看实际进度；构建耗时受 runner、网络及依赖缓存影响。

---

## 流水线结构

```
meta（版本门禁）
  └─ gate（复用 ci.yml：cargo test --workspace ×2 + ts-rs 门禁 + tsc + UI 回归 + vite build）
       ├─ windows（daemon release → sidecar → 壳身份回归 → tauri build → msi/nsis artifact）
       └─ docker （buildx → ghcr.io，tag 版本号 + latest）
            └─ release（下载 artifact → 建 GitHub Release，自动 changelog）
```

后端 workspace 测试在 Ubuntu 跑。Windows 作业放置 sidecar 后，串行执行独立
`src-tauri/` 工程的服务身份与产物解析回归，再构建安装包。两工程各用自己的
`target/`，不会把正在运行的 daemon 当构建输出覆盖。

Windows job 缓存 Cargo `target/`，其中也可能留有先前构建的 bundle 文件。便携运行目录按 workflow run ID/attempt 唯一命名；上传 ZIP 写在 workspace 根目录，避免命中缓存中的旧目标目录，同时保持 Release 附件名稳定。

桌面壳只接入 `/health` 中 `product=jev-switch`、`api_revision=1` 且版本与壳一致的
内核。CI 将提交 SHA 写入 `JEV_BUILD_REVISION`，用于关联运行产物；它不包含密钥。
没有身份字段的旧 Demo 不能因 HTTP 200 而被新版壳误接入。

---

## Windows 最终连续验收：先便携运行，必要时再升级安装版

先完成源码门禁、容器验证、MSI/NSIS 构建与便携目录组装；便携目录和 MSI 必须来自同一次 Tauri build，并核对壳、daemon、HTML/JS/CSS 的 SHA-256。Tauri daemon 是 `externalBin`，所以便携运行时是一个带 `jev-switch.exe` 的文件夹（含 daemon 与 `ui/dist`），不是可以脱离资源目录的单文件 exe。日常 UI、路由、真实调用、历史与恢复验证优先运行便携壳，避免每轮改动都触发安装/UAC。

只有需要专门验证 MSI 安装器、安装路径升级行为或安装器迁移时，才在便携验收完成后做一次 MSI 升级；将该步骤与最终运行验收安排在同一轮。候选 MSI、便携目录、正式配置保护措施、预期请求与截图视口都应提前备齐。

### 触发安装前

- 确认当前提交的 Rust workspace 默认/`ts-rs` 测试、Tauri shell 测试、UI 测试、lint 与生产构建均通过。
- 运行 `scripts/build-windows-release.ps1` 一次生成 MSI、NSIS 与便携目录。便携目录内含可双击的 `jev-switch.exe`，并带 daemon、`ui/dist`、`build-manifest.json`；清单记录壳/daemon/UI 和配对 MSI/NSIS 的 SHA-256。重复构建会生成新的便携目录，不覆盖旧验收产物。不要将 AppData 密钥或配置复制进便携包。GitHub Release 同样上传一个便携 ZIP，解压后运行目录中的 exe。
- 从 MSI 只读提取 Tauri shell、daemon 与 HTML/JS/CSS，逐项与当前构建输入哈希核对。之后常规 UI、路由、真实调用、历史与恢复验收直接双击便携目录中的 exe；仅安装器升级专属验证才需要 MSI/UAC。
- 核对现有 Jev、daemon、Laya 与监听端口归属，避免覆盖或停止其他项目服务；确认现有 Vercel 凭据只通过掩码状态检查，不读取或记录明文。
- 准备演练场相同输入、公开入口与直连 provider/model 两类目标；准备 Dashboard request ID、SQLite event/trace 字段核对项。

### 默认：便携运行验收

1. 核对 `127.0.0.1:11435` 的监听 PID 与路径。只在确认属于 Jev-Switch 后结束占用，再双击便携目录内 `jev-switch.exe`；核对启动的壳、sidecar、health/API identity 与 `build-manifest.json`。若 shell 复用了原 daemon，先确认它确为本次目标版本，不能只看窗口已打开。
2. 用现存正式配置核对 schema migration、入口/provider 状态及既有历史计数；不覆盖配置、不打印密钥。
3. 各执行一次真实 Laya 与 Vercel TypeSafe 公开入口请求，并通过演练场各执行一次直连上游比较。把响应 request ID 与 Dashboard 活动及 SQLite 记录对应，确认成功/失败状态、provider/model、usage、latency、route trace 按契约保存；确认请求正文、答案、key 与原始错误 body 未写入安全 trace。
4. 在便携版或升级后的 WebView 检查 Dashboard、Providers、Routing/入口 DAG、Playground 多入口并排四页；覆盖短窗口、窄屏长宽比与现有多 DPI 显示器。确认布局使用可用视口，入口图与实际配置一致，比较结果并排可读。
5. 通过真实托盘菜单执行“退出”，确认 shell 负责停止其拥有的 sidecar；重启后核对入口、配置、历史和统计恢复，再核对关闭/隐藏到托盘行为。
6. 全部检查通过后再截取当前候选版本的四页正式截图，更新 README/发布材料与验证记录；截图不得来自旧 demo、开发预览或旧 UI hash。

### 可选：一次 MSI 安装器升级验收

若本轮确实要验安装器，在便携运行与页面/路由检查完成后使用同一构建的候选 MSI 升级，并在这一处处理 Windows UAC。确认 ProductCode/UpgradeCode、安装目录与 AppData migration；不要为每次 UI 或 daemon 修复重复安装。MSI 成功后，按同一 request ID / SQLite / 页面检查再抽查一次即可。

有一项失败时，记录结果并停止本次发布验收；优先在便携运行目录修复和复测。只有安装器专属问题才重新安排一次 MSI 验收。当前正式发布目标为 `v0.1.0`；本文中的 `0.6.0` 只作语法示例。

---

## 干跑（不发布，只验流水线）

Actions → Release → Run workflow（`workflow_dispatch`）。
构建 Windows 产物 + 验 Dockerfile 能 build，但**不推 ghcr、不建 Release**。
`version` 输入留空则取 `ui/package.json`，做四处一致性检查。
输入通过环境变量传入门禁，不作为 shell 源码拼接；不一致的输入被拒绝。

---

## 产物在哪

| 产物 | 位置 |
|---|---|
| MSI / NSIS | Release 页附件；或 Actions run 的 `jev-switch-windows-<版本>` artifact |
| Docker | `ghcr.io/<owner>/<repo>:<版本>` 与 `:latest`（owner 自动转小写） |

发布页自动填入实际的小写镜像名称与版本，并提供带持久卷、管理员密码和 15 秒停机预算的启动命令。Bash 示例中的 `0.6.0` 仍只是示例，先替换 `<owner>/<repo>` 和版本，再将 `JEV_ADMIN_PASSWORD` 设置为自己的管理员密码：

```bash
: "${JEV_ADMIN_PASSWORD:?请先设置管理员密码}"
export JEV_ADMIN_PASSWORD
docker pull ghcr.io/<owner>/<repo>:0.6.0
docker run -d --name jev-switch \
  -p 127.0.0.1:11435:11435 \
  --restart unless-stopped --stop-timeout 15 \
  -e JEV_SWITCH_MODE=cloud -e JEV_BIND=0.0.0.0:11435 \
  -e JEV_ADMIN_PASSWORD -v jev-switch-data:/data \
  ghcr.io/<owner>/<repo>:0.6.0
```

打开 `http://127.0.0.1:11435`，使用管理员密码登录，再到 Token 管理创建调用 Token。示例只发布到宿主回环；远程部署、反向代理和配置导入按 [部署文档](deployment.md) 操作。命名数据卷保留数据库，容器替换不等于删除数据卷。

---

## 仓库一次性设置

Settings → Actions → General → Workflow permissions → **Read and write**。
`GITHUB_TOKEN` 自带 `packages: write`，推 ghcr 无需额外 secret。

---

## 失败排查

| 症状 | 原因 / 处理 |
|---|---|
| `meta` 报版本不一致 | 四处版本号没同步，按上面清单改齐重新打 tag |
| `gate` 报 ts-rs 生成物不一致 | 跑 `cargo test --manifest-path rs/Cargo.toml --workspace --features ts-rs` 后提交 `ui/src/generated` |
| Tauri 找不到 sidecar | `src-tauri/binaries/` 已 gitignore；流水线的「放置 sidecar」步负责生成，本地构建需手动跑（见 `docs/deployment.md` §9.2） |
| MSI 构建失败提 ICE30 | sidecar 与壳主二进制同名了。必须叫 `jev-switch-daemon-<triple>.exe`（`docs/deployment.md` §9.4-2） |
| `beforeBuildCommand` ENOENT | hook 的 cwd 是仓库根，用 `--prefix ui` 而非 `../ui`（`docs/deployment.md` §9.4-1） |
| ghcr push 403 | 仓库 Actions 权限没给 write（见上节） |
| `__TAURI_BUNDLE_TYPE` warn | 无 updater 插件的已知无害告警，忽略 |

---

## 尚未做

- macOS / Linux 打包（Windows 首发裁决，`docs/12` §三冻结）
- 代码签名与公证
- Tauri updater 增量更新
