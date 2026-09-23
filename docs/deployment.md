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

- 配置真值：`./data/providers.toml`（宿主 bind mount；**首启自动播种**为 `rs/providers.example.toml` 的拷贝）。用目录挂载而非单文件——宿主文件不存在时 Docker 会误建同名目录。
- 打开 `http://<主机>:11435/` 即控制台（daemon 同源托管 `ui/dist`，API 走相对路径，无跨源）。
- cloud 态凭据缺失 = **fail-closed**：无 token → 全 `/v1` 401；无 admin 密码 → 登录恒 401（启动日志有 warn）。**两个值必设**。

本地非容器跑法不变（README 快速开始三命令；默认 `mode = "local"`）。

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

---

## 三、local / cloud 两态对照

| 维度 | `mode = "local"`（默认） | `mode = "cloud"` |
|---|---|---|
| 绑定 | `127.0.0.1:11435` | `0.0.0.0:11435`（容器内必需，见 §五偏差） |
| `/v1/systemone`、`/v1/models` | **完全不校验**（现状零变化） | `Authorization: Bearer <调用 token>`；缺失/错值 → **401** |
| `/v1/admin/*` | **完全不校验** | `POST /v1/admin/login {password}` 换**短时会话 token**（内存态、2h、不落盘）→ 后续带 `Authorization: Bearer <会话>` |
| `/health` | 放行 | **放行**（探活需要，不设门） |
| 凭据来源 | —（不需要） | token：`auth_tokens`（toml）或 `JEV_AUTH_TOKENS`（逗号分隔，**非空 env 完全覆盖**）；密码：`admin_password`（toml）或 `JEV_ADMIN_PASSWORD`（**非空 env 优先**） |
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

1. **无 token 即 401**：cloud 态下 `/v1/*` 一切请求先过 Bearer 门。token 列表为空 = 全部 401（fail-closed，不是放行）。把 token 当密码管：`.env` 已 gitignore；**不要**贴进 issue/截图/日志；轮换 = 改 `.env` + `docker compose restart`。
2. **admin 密码必设**：未设置时 `POST /v1/admin/login` 恒 401（fail-closed），管理面等于锁死——这是故意的。密码与调用 token **分权**（双 token 裁决，docs/12 §二.6）：调用 token 进不了 admin 门，会话 token 也进不了 `/v1` 门（有单测断言分池）。
3. **会话是内存态**：不落盘、2 小时过期、重启全废。浏览器侧只存会话 token（`localStorage["jev_admin_session"]`），**密码从不落浏览器存储**。
4. **防火墙建议**：
   - `0.0.0.0:11435` 绑定 = 所有网卡可达；云主机请在安全组/防火墙**按需放行**（自用可只放行办公网 IP 或走 SSH 隧道）；
   - 暴露公网时**前置 HTTPS 反代**（nginx/caddy 终结 TLS）——Bearer 明文头在公网上必须加密传输；本仓 CORS/鉴权不管传输层加密；
   - 非必要不开 `0.0.0.0` 到无关网段；本地态（loopback）永远是更安全的默认。
5. **redact 兜底**：调用 token 与 admin 密码已纳入 daemon 的 known_keys 集——错误体/日志若意外拼进凭据值会被掩码（contracts/04 §2 红线 4 扩展面）；但**别依赖兜底**，上游侧日志仍须自己管住。
6. **探活无鉴权**：`/health` 两态放行——它只回 `{status, version}`，无敏感信息；这是探针（Docker HEALTHCHECK / LB）的必要豁免。

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
1. **同源消解 CORS**：UI 与 API 同一 origin，浏览器零预检、零白名单问题（反代改端口需设 `window.__JEV_BASE__`，见下）；
2. **单进程单端口**：镜像无第二个常驻进程，HEALTHCHECK/日志/重启语义简单；nginx 方案要多维护一个反代配置面；
3. 本项目 CORS 白名单语义（5173 dev + 同源 prod）已够用——nginx 在这里只搬运字节，不带来新能力。

代价与边界：TLS 终结、限流、访问日志切割等仍建议在云上**外层**反代做（§四.4）——那是传输层/边缘职责，与本选型不冲突。

dist 路径由 `JEV_UI_DIST` 指定（镜像内 `/app/ui/dist`；本地默认 `ui/dist`）。dist 缺失时 API 端点不受影响（fallback 只吃未匹配路由）。

---

## 七、给调用方 / UI 线的接口备注

**HTTP 头（两枚 token 分池）**：

| 场景 | 头 |
|---|---|
| cloud 调 `/v1/systemone`、`/v1/models` | `Authorization: Bearer <调用 token>`（`auth_tokens`/`JEV_AUTH_TOKENS` 之值） |
| cloud 调 `/v1/admin/*` | 先 `POST /v1/admin/login` `{"password":"…"}` → 200 `{"token":"…","expires_in":7200}` → `Authorization: Bearer <token>` |
| 401 错误体 | `{"error":"…","upstream":null,"retryable":false}`（与契约 05 §3 同形，经 redact） |
| `/health`、`POST /v1/admin/login` | 不带任何鉴权头 |

**UI 存储键（已实现）**：
- 调用 token：`window.__JEV_TOKEN__` 或 `localStorage["jev_token"]`（api.ts 读取；**未设则不带头** → local 态零变化）
- admin 会话：`window.__JEV_ADMIN_SESSION__` 或 `localStorage["jev_admin_session"]`（api/admin.ts 读取；login 成功自动写入）
- 反代到非 11435 端口时：页面加载前设 `window.__JEV_BASE__ = "https://你的域名"`（api.ts 的 getBase 优先级最高）

**浏览器现状**：Providers/Routing 页任意 admin 401/403 → 自动弹登录小窗（Shell 全局接线）；登录成功整页刷新重拉。local 态永不触发（服务端不产 401）。

---

## 八、验证入口（本任务门禁）

| 层 | 命令/断言 | 状态 |
|---|---|---|
| Rust 全测 | `cargo test --manifest-path rs/Cargo.toml --workspace`（含 auth 中间件/login/config 双态专测） | 见提交记录 |
| UI 门禁 | `npm run lint --prefix ui` && `npm run build --prefix ui` | 见提交记录 |
| cloud 冒烟（本机 daemon，等价 compose 验收） | `JEV_SWITCH_MODE=cloud` 起 daemon → 无 token 401 / 有 token 200 / login→admin 200 | 见提交记录 |
| compose 实跑 | `docker compose up -d` → curl → down | **本机无 docker/podman，跳过**（报告注明；中间件已由单测全覆盖） |
| local 回归 | `scripts/smoke.ps1`（免 token 现状） | 见提交记录 |
