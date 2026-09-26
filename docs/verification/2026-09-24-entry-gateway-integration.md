# 入口网关实施与联调记录（2026-09-24）

状态：实施中。本记录是分项证据，不是 P1–P6 整体验收。范围以 [总计划](../design/ENDPOINT-GATEWAY-ALIGNMENT-PLAN.md) 为准。

最新证据见文末发布准备、Windows 同版本隔离联调、三屏几何、NSIS 实装与 MSI 机器级实装检查点。旧段中的“当前/最新/待验证”均限于该段记录时点，不能覆盖后续证据。P1–P4 的实现与 Web 产品范围已收口；Windows 内核独立 Web 联调、三屏首窗几何、解包包冷启动、NSIS currentUser 实装，以及 MSI perMachine 实装与安装版启动均有实测。隔离 Tauri 壳现已在真实安装版 daemon 上复用并加载本地 UI；实际 MSI 主壳完整生命周期、托盘退出及真实 Vercel 上游调用仍待验。P5/P6 未收口。

> **历史记录说明（2026-09-27）：**上段状态是本记录形成时的阶段性结论，后续 v0.1.0 发布与收尾证据已改变其中的“待验/未收口”状态。不要将这份逐时记录单独用于判断当前完成度；当前发布边界、剩余事项和验收依据以[入口网关认知对齐与实施计划](../design/ENDPOINT-GATEWAY-ALIGNMENT-PLAN.md)及[发版说明](../RELEASE.md)为准。以下各节保留当时的时间线证据。

## 已实现并定向验证的前端内容

- Routing 已接入对外入口卡片/编辑器与调用 DAG。左侧对外入口、中间路由节点、右侧按接入配置分组的模型端口；支持拖动、磁吸、双端重连、属性编辑、插入/删除中间节点、完整文档撤销/重做与键盘接线。仍需完成全部真实交互回归。
- 保存按单次在途请求串行推进，保留新编辑；失败保留草稿。服务端可以重排存储行，但不得丢边或改变属性；保存完成须独立 GET 回读核对，不能只相信 PUT 回显。
- Providers 复合卡支持 name/account/models、同地址不同凭据分卡；普通全表更新保留新增字段。Probe 显示连接可达性，与模型推理健康分开。
- Playground 使用独立队列、取消与 attempt 标识，支持对外/直接上游目标；已将调度回归迁入仓库，仍待三种目标组合的完整端到端验证。
- 多语言使用注册表及选择列表，当前仅 en/zh；浏览器语言匹配由注册表驱动。后台独立浏览器标签实测中英切换、刷新后保留及 HTML lang 更新通过。

`npm test --prefix ui` 最新通过 15 项：多跳布局、同地址双账号与不同模型连线、40px 磁吸、完整属性与位置撤销、空入口、回读差异、比较队列并发与取消/迟到响应/重新挂载、TOML 模型数组解析、输入快照/字段错误、重连后卡片位置稳定，以及调用凭据只在内存中保留和身份切换。测试在 `ui/tests/`，并已加入 CI 前端作业。AuthContext 曾出现身份 API 导出与登录函数类型错误，已修正；最新 `index-DkjaWIdY.js` / `index-Cud6Qbhs.css` 构建通过，包含认证整合、生成 DTO、五字段健康身份及紧凑输入布局。类型检查通过仍不等于 Token 浏览器权限验收完成。

提供商页已接入配置文件同步区（状态查询、外部修改提示、显式导入/导出），DAG 已增加当前浏览器会话的未保存草稿恢复，均包含在最新静态构建中。草稿跨页恢复已有实测；配置文件操作及 Token/活动 UI 仍须完成真实权限与数据联调。

## 隔离运行来源

只使用测试数据与受控上游，不使用个人真实 API key。配置：`E:/tmp/jev-ui-acceptance/providers.toml`；独立数据目录：`E:/tmp/jev-ui-acceptance/data`；受控上游脚本：`E:/tmp/jev-ui-acceptance/upstream.mjs`，监听 18766，输出明确为测试固定值，不能作为真实模型质量证据。

前端 Vite 5173 后转用 daemon 11435 托管的 `ui/dist` 静态构建，避免并行施工热更新影响操作。保留同地址 personal/team 两个配置，每个配置具有 M1/M2。后续更换内核须记录准确产物，不能把磁盘源码更新当作运行中进程已更新。

## 实际发现及处置

1. **开发页面热更新异常。** Edge 的 Vite 页面在并行热更新期间出现 `removeChild` 异常并丢失编辑弹窗；页面存在翻译扩展标记，但没有证据将原因单独归于扩展。冷加载静态构建后，已操作的创建流程未复现此异常；未宣称一般性 HMR 问题已修复。
2. **入口列表死锁。** UI 创建 `jev-fast → personal/M1` 后，自动 GET 入口列表无响应，连 `/health` 也在 3 秒内无法返回。`list_endpoints` 持有 SQLite mutex，又在 `count_routes` 重入同一锁。后端已提前释放 guard，新增创建后列表读取及健康检查回归；新进程复验列表恢复、原入口从独立 DB 保留。
3. **双来源路由覆盖。** 实际拖到 M2 端口附近松手创建第二条连线，修改优先级与 sticky，再插入 `fallback-main` 后，图上是三条边，但较早内核 GET 仅剩两条，缺少 `jev-fast → fallback-main`。调用仍返回 M1，不能按图形或 PUT 200 验收。后端继续统一快照与入口事务，前端增加独立回读检查。
4. **产物版本差异。** `E:/tmp/jev-switch-cargo-20260924/debug/jev-switch.exe` 的 21:09:55 构建早于后续快照/事务修订；该产物也复现第 3 项。此结果只证明该旧产物的问题，不冒充对最新源码的结论。须等待明确的新产物并重跑完整操作链。
5. **新快照产物与优先级语义分开验证。** 21:25:00 构建的 SHA256 为 `99D3F5692F175ED07A5B143FE2E945550C247250BF1A81ED4540974ADFFCFEFC`，复制到隔离目录运行后，管理 API 和实际浏览器都已返回完整三条边，入口路由数为 2。但调用仍命中 M1：旧 core 按末跳 priority 全局排序，将两条末跳 priority=10 的候选并列，忽略根分支 7 优于 10 的含义。这与契约 03 的“同 left 下越小越优先”不一致。后端已改为逐层优先级遍历并通过针对性测试，尚待对应新产物真实复验；不能把此次 M1 简单归为快照仍丢边，也不能用图已恢复宣称调用语义已正确。
6. **公开集合边界。** 99D3 产物的 `/v1/models` 仍列出中间节点 `fallback-main`，演练场会误将其当公开候选。后端正在改为仅发布启用的服务入口，并为旧 TOML 的根入口做一次兼容迁移；中间别名不能单独绕过入口配置调用。
7. **前端操作细节。** 真实重连后提供商卡片曾因路由行顺序改变而交换位置，现已改为稳定对象顺序并有回归。比较面板已选目标仍显示空目标提示、无 key 本地上游被前端禁选、无效 questions 被误报成 state 错误，也已修正。表单现在阻止重复选项键静默覆盖；在真实页面改出重复键后，显示错误并禁用“运行全部”，修复后恢复可用。

## 已取得的浏览器证据

- Edge 独立标签、1592×721：personal/team 两卡分别呈现，两列各 608px；各有 M1/M2，只有密钥遮罩，Probe 明示仅探测连接，页面无横向溢出。语言在 English/简体中文之间切换及刷新持久化通过；截图接口超时，本项以 DOM/几何与实际控件操作为证，未捏造截图。
- Codex 内置浏览器：确认公开入口、提供商卡与模型端口可见；实际鼠标在 M2 端口附近松手成功磁吸。键盘 Enter 依次选择源/目标端口也能创建连线。插入中间节点后本地三边结构正确；保存到执行的一致性仍由第 3、4 项后续复验决定。
- 同一浏览器已实际将多跳末端从 personal/M2 拖到 team/M2，并撤销恢复。起点重连、完整重做、环拒绝和失败草稿保留仍待覆盖，不因纯函数测试通过而提前勾选。
- 混合比较已实际执行 `jev-fast`、personal/M1、team/M1 三列，均独立完成。受控上游记录验证三份 state/questions 完全一致，账号归属分别为 personal、personal、team；固定 Noul 值分别为 0.73、0.73、0.61。该证据证明输入与账号隔离的执行链，不证明真实模型准确性，也不替代修正后的策略验收。
- 中文表单题面、判断标准、选项计数与无障碍标签及主题按钮已实际呈现中文。手动制造无效 questions JSON 后正确显示对应字段错误；未知费用保持“未知”。尚未将其他语言列为可用。

## 下一步门槛

- 在包含最新统一快照修订的明确内核产物上，重复“入口创建 → DAG 多跳编辑 → 入口读取/修改 → 实际调用 → 重启恢复”。检查禁用入口的路由仍能被管理端读取，且不会被下一次全表保存误删。
- 验证重连两端、撤销/重做、环拒绝、失效引用、保存失败和输入框快捷键保护；宽窄屏均做实际操作。
- 完成其他策略、Token 管理/权限隔离、真实调用事件/统计与多目标演练联调。分 Token 管理是用户已批准专项；共享入口策略不意味着取消 Token 管理。
- 同版本 Web/Tauri/Docker 验收、正式截图、文档与既定发布仍按 P5/P6 收尾，当前不勾选完成。

## 22:00 后的明确产物复验

### 逐层优先级、启停与重启恢复

Windows daemon SHA256 `2A5BFBCD1702C0B0F147EE67138C28C07CE44912DCE60680C077EA76C45C3C5C`，复制到 `E:/tmp/jev-ui-acceptance/bin/jev-switch-2a5bfbcd.exe` 后运行。直接调用证据修正了前文 99D3 检查点的遗留项：

```text
jev-fast
  +-- priority 7  -> fallback-main -> priority 10 -> personal / M2  [实际命中]
  +-- priority 10 -> personal / M1
```

- `/v1/systemone` 实际返回 M2；`route_trace.selected_hops` 为 `fallback-main/personal`，受控上游日志也记录 personal/M2。
- `/v1/models` 仅发布 `jev-fast`；直接调用 `fallback-main` 返回 404。
- PUT 入口只更新 enabled、省略 routes：禁用后公开集合为空、调用 404，管理端仍读出完整三条边；恢复启用后继续命中 M2。
- 停止已核实的测试进程，再用同一不可变二进制/配置/数据目录启动：三条边和 M2 命中保持。该链路通过，不能再沿用“最新运行产物仍丢边”的旧结论。

随后升级到 SHA256 `933CB46AD7D140CC2742D8B7D50DC009B5A155001BC34FA07D1695EC8F5D5623` 的 Token/health 检查点（不可变副本 `jev-switch-933cb46a.exe`）。健康接口返回 product `jev-switch`、api_revision `1`、version `0.1.0`、build_revision `null`；已有三个入口及真实调用计数被保留。未设置构建修订时保持 null，不冒充提交版本。

### 真实本地模型与三列比较

`E:/venvs/laya/Scripts/python.exe` 启动仓库 `scripts/laya_multi_server.py --host 127.0.0.1 --port 18767 --device cuda`，设置 Hugging Face/Transformers 离线模式。使用已存在的 snapshot `1c5edc17a7acd8701df6fc341c0d179f1c62c982`，健康查询确认 english/multilingual 已加载。没有新下载权重，也没有使用个人云服务凭据。

通过提供商管理 API 新增 `laya-local`（本地地址、无 key、两个模型），创建 `jev-local-en` 与 `jev-local-zh`。保留原有 personal/team 测试配置和路由，因此图扩展为五条边。

- 真实 API 调用覆盖 choice/score/noul 三类题；英文公开入口与 Multilingual 直调均成功，usage/分布/置信度来自模型，费用未知保持 null。
- 浏览器执行 `jev-local-en`、`jev-local-zh`、`laya-local/laya-english` 三列，前两列经过公开入口，第三列直接调用指定模型。使用内置三题示例 `Multi·Churn Risk`。
- 公开 English 与直接 English 的分布、score、noul 相同；Multilingual 的分布不同。例如 English 的 score=1.137，Multilingual=1.406，界面保留差异，不以网关常数覆盖结果。这是传输/归属和比较能力证据，不是模型准确率基准。

### DAG 失败处理与空间回归

实际拖动 `fallback-main → personal/M2` 的起点到 `jev-local-en`，撤销再重做后仍保留 priority=10、M2、session 和 on_error=next。由于此测试暂时让 fallback-main 没有出边，后台明确拒绝无效引用；前端保留失败草稿，离开到 Providers 再返回时显示恢复提示。放弃草稿后回到服务端正确配置。

进一步真实操作：在输入框按 Delete 不删除连线；连接 fallback-main 回 jev-fast 时识别环并阻止写入；上下文菜单删除中间节点及两条相连边，再撤销恢复。最终独立 GET 回读五条边，根分支 priority=7 与末边完整属性都正确。

三题表单最初仍会撑高右侧、在左侧下方留下大片空白。根据实际截图修正为默认紧凑输入区，两侧独立滚动，可展开编辑；不只把结果区改成多列。

- 1440×900：输入区 288px 高，问题滚动区可视 242px/内容 1106px，三个结果卡约 455px 宽、同一行起点。
- 390×844：文档宽 382px，三个结果卡各 348px、纵向排列；无整页横向溢出。展开后问题区 480px，键盘可滚动到后续题目。
- 这些截图用于调试核验，未提前替换用户要求在最终验收后统一更新的正式展示素材。

### 交付链正在推进

Tauri 壳已增加服务身份/接口修订/版本检查；仅随包 daemon 名称可供发行包解析，排除壳自身和工作目录中的旧文件；数据目录显式与配置实例一致，启动失败可从配置目录的 daemon.log 排查。三个有针对性的壳测试通过，完整桌面构建和冷启动/复用仍须运行验证。

Windows PATH 没有 Docker，但 WSL Ubuntu 中实际存在 Docker 28.4.0，使用本机 `unix:///var/run/docker.sock`。已据此启动真实容器构建检查；不能将 Windows CLI 缺失误写为 Docker 验证不可进行。Docker 默认 cloud，并显式持久化整个 /data；最终镜像启动、鉴权、重启恢复及发布仍待完成。

## 后续检查点：2026-09-25 交接

本节更新前文“最新”的时间边界，旧记录保留为历史证据。

### 内核、权限与计量

新 daemon SHA256 为 `A2BD71B44F9AD70575CF8D7E9050EB97D8CF5553E0DA0AC0A0A2CBDB7C99D020`（构建时间 2026-09-24 15:12:28 UTC，18,895,360 bytes）。11436 cloud 测试实例已使用该检查点；11435 仍运行前文 933CB46A 副本，不能混用两者的验证结论。

- 后端 `cargo test --workspace` 与 `cargo test --workspace --features ts-rs` 均通过。核心计数：adapter 23 + wiremock 2、core 56、daemon lib 70、endpoints_runtime 5、integration_http 8、listen_rebind 7。隔离构建目录为 `E:/tmp/jev-switch-ci-luna`。
- 57 个生成文件在 ts-rs 测试前后 SHA 相同；当前相对 HEAD 的生成类型差异来自尚未提交的接口修改，不是此次生成漂移。提交时须连同源码保存，再在干净 checkout 上执行 CI 差异门禁。
- cloud HTTP 验证覆盖 admin/readonly 身份、Token 创建/修改/停用/恢复/撤销、停用及撤销后 401、自身统计和调用记录、拒绝跨 Token 查询、拒绝 readonly 管理权限，以及管理端/自身 SSE。Token 列表不回传一次性 secret。A2 检查点升级后身份、入口及已有统计保持。
- Race 只统计实际进入 adapter.evaluate 的请求；胜出后取消尚未完成的任务。Shadow 保证影子任务已进入 evaluate 后再启动主路。调用响应中的 usage/cost/latency 属于返回的上游结果；网关总耗时另记为 `route_trace.gateway_latency_ms`，多请求的未知合计费用保持未知。迁移 `004_call_usage_observability.sql` 保存调用计量。
- 个人调用记录持久化；管理端全局事件流仍是容量 200 的内存 EventBus，不能称为完整持久审计历史。详见契约 07。

### cloud 浏览器和同源部署

实际 11436 页面暴露了生产 UI 默认连错 11435 的问题。`api/base.ts` 已改为生产 HTTP(S) 页面默认同源，开发模式保留 11435，显式 `__JEV_BASE__` 仍可覆盖。新测试覆盖自定义端口、HTTPS 反向代理、开发与非 HTTP 环境，前端测试现为 17 项。

11436 使用 `E:/tmp/jev-responsive/dist`，已在真实浏览器完成管理员密码登录；页脚正确显示 11436。全局策略参数按顺序保存并重新打开核实：race timeout=1500、load_balance weight=equal、shadow target=team，最后恢复 failover。登录弹窗取消与重新打开均可操作。

管理员密码会话仅用于管理，公开模型与公开调用仍需要调用 Token。演练场此前将 401 误显示为空模型集合；现已分开 loading/ready/empty/401/403/network/other HTTP error，并提供前往 Dashboard“我的用量”输入调用 Token 的路径，没有扩大后端权限。该修复 lint 和 17 项测试通过，最新静态构建为 `index-uiXSIxOV.js` / `index-BIQ2wcjF.css`；浏览器复验在下文续记。

### 首窗与响应式适配

用户新增要求已落入总计划 §3.2。默认首窗改为 1000×650 逻辑像素，常规最小 760×480，必要时按当前显示器工作区/DPI 与窗口边框缩小，以免初次启动超出屏幕。配置先隐藏窗口，应用尺寸和居中位置后再显示。

常规 Tauri 回归现为 5 项。额外显式执行 `native_initial_window_stays_inside_current_work_area`：创建独立隐藏原生 WebView，不启动 daemon/托盘/单实例服务；实测工作区 1600×852、scale=1、内容区 1000×650、外框 1016×689，位置 (292,82)，完整位于工作区内。缩放比例 125%/150%/200% 的预算算法有单元测试；真实多 DPI 显示器与跨屏移动尚未实测。

页面已覆盖 320/390 窄宽、760×480、1000×650、1280×400 矮窗以及 1920/2560 宽屏。修复了窄屏登录区重叠、入口弹窗内容压住底部按钮、提供商卡片留空和跨页继承旧滚动位置。详细 DOM 几何、控件操作和局限见 [响应式验证](../responsiveness-check-2026-09-24.md)。调试截图未替换正式展示素材。

此前“复制 bundle、停止现有 Tauri/daemon 并启动新壳”的组合命令被自动审批审查拒绝，唯一返回理由为 `blocked by policy`，命令未执行，也未拆分重试。上述独立窗口测试不停止或替换现有实例，仅证明新窗口初始化逻辑；完整安装包冷启动、随包 sidecar 及退出生命周期仍未验收。

### Docker：失败范围明确

首次构建暴露 UI 阶段缺少共享 JsonValue 生成类型，已补 `rs/crates/jev-protocol/bindings` 的 COPY。随后 Debian HTTP 包索引连接超时；改为 HTTPS，并从同 Debian 系列的官方 Rust 构建镜像引入信任根，未关闭证书或仓库签名校验。

- `E:/tmp/jev-ui-acceptance/docker-build-https.log`：Linux Rust release 编译成功（6m33），APT 索引获取部分成功，最终因 `deb.debian.org:443` 超时失败。
- `E:/tmp/jev-ui-acceptance/docker-build-host.log`：诊断性 host-network 构建仍在相同 CDN 的 Packages 索引超时，失败退出。
- 两个构建会话均已终止，没有可验收的最终镜像或运行容器。后续须先改变并验证包下载条件，不能重复同一长构建或把网络失败写成代码验收成功。

P1–P4 仍按各项操作证据逐步收口，P5 的完整 Tauri 与 Docker 验收、P6 的正式截图/提交/CI/发布仍未完成。

### 01:45–02:00：只读实操暴露的游标问题与修复

使用隔离测试用的 readonly Token（ID `tok_68a688709eeb05a1cc1d`，不在文档记录 secret）完成浏览器登录：顶部仅保留 Dashboard/Playground，提供商直调控件不显示；M1/M2 公开入口横比均成功。两列分别报告客户端 131/284ms、网关 110/254ms、上游调用数各 1，费用未知。个人统计为 2 次、平均 182ms、错误率 0，符合这两次真实请求。

但活动表实际显示四行：历史查询从 `call_logs` 取 ID，个人 SSE 从内存 EventBus 取 ID。两个独立序号空间合并后出现重复，还可能把游标推进到错误的位置。已将个人 SSE 与历史查询统一为 `caller_events`，使用同一数据库记录、detail 和 `since`。数据库失败时关闭流，让客户端显示错误/回退，不能伪装成正常空列表。

- 新增回归测试覆盖数据库 ID 从 101 开始、其他 Token 插入 102、历史/SSE 完整 JSON 相等、从 101 重连不重播、后续本 Token 的 103 正常到达且不混入另一 Token。
- daemon 完整测试通过：lib 71、endpoints_runtime 5、integration_http 8、listen_rebind 7。未改 DTO，无须变更生成类型。
- 新二进制 SHA256 `2DBB3D3ED8FD681967A44E2649B6467E7D00ADB00C7B92734504F23B3EFF17CD`，复制为 `E:/tmp/jev-access-acceptance/bin/jev-switch-2dbb3d3e.exe`。核对 11436 端口和旧 A2 副本的进程身份后，仅更新该 cloud 测试实例；配置与 `data-cloud` 保持。11435 和原 Tauri 进程不变。
- 实际浏览器重新进入个人活动：两次调用只显示两行，已有统计保留；随后外部发起一次同 Token 请求，实时流只新增一行。清除 Token 后个人数据区域消失；刷新页面不保留内存调用凭据。
- 英文 readonly 页面在 390×844 时 document.scrollWidth=390，导航、身份、个人统计与活动表可访问。统计为加载时的时间范围快照，已补可随时使用的用量刷新按钮，不要求重新登录才能更新统计；追加第四次调用后，真实点击用量刷新得到请求数 4、平均 179ms、错误率 0，活动表也是四行。结束时恢复中文与默认视口。
- 最新 UI 为 `index-B7kICrc5.js` / `index-BIQ2wcjF.css`，lint、17 项测试和生产构建通过。公开模型 401 提示有可操作的 Dashboard 路径，并且不再同时误标“已发现”。

Docker 网络条件已取得新证据：[Debian 官方镜像列表](https://www.debian.org/mirror/list) 中的 USTC 镜像在真实容器内通过 HTTPS 下载 8,790,396 bytes 的 Packages.xz，HTTP 200、3.47 秒。Dockerfile 增加可选 `DEBIAN_MIRROR` 构建参数，默认仍为 Debian CDN。以 USTC 进行的新构建 APT 阶段 12.4 秒通过，日志为 `E:/tmp/jev-ui-acceptance/docker-build-ustc.log`；随后 cargo 仍在获取 crates.io 索引，进程已确认存活。此构建的源码快照早于上面的个人 SSE 修复，即使成功也须更新产物后再宣称最终同版本验收。

## Windows 安装包与剩余交付门槛（2026-09-25）

### 配置文件管理与管理员调用身份

在 11436 cloud 页面以管理员密码登录后，实际执行“将当前配置写入文件”。导出文件保留在内核机器，页面只显示掩码。随后将测试提供商 `team` 的名称从 `Laya Team` 改为 `Laya Team Import Check`：刷新同步区会显示外部修改，但提供商卡仍为原名；显式导入成功后卡片才更新。

向测试 TOML 追加不完整表头后，再次导入被拒绝；页面显示 parse error，原有卡片和已生效配置保留。恢复导出时的文件并在 UI 重新导入后，卡片恢复 `Laya Team`。独立 API 回读两提供商启用、M1/M2 两入口仍启用且 follow_global，证明入口策略没有被文件同步覆盖。备份位于 `E:/tmp/jev-access-acceptance/providers-before-sync-20260925.toml` 与 `providers-exported-20260925.toml`，不进入仓库。

管理员角色调用 Token `tok_0108f255067a375912ae` 的浏览器登录已验证：可查看多个 Token 的用量汇总，导航保留管理功能，可查询并执行 M1/M2 公开入口。凭据没有写入文档。

### 演练场路径展示

计划要求展示实际路径，原实现主要放在原始响应中。现已增加可读的选中路径、路由策略、已发候选详情和 Shadow 已发状态（不虚报影子完成）。字段直接取自 `route_trace`，缺失时说明内核未提供路径。

真实 M1/M2 两列分别显示 `M1 → personal / M1` 和 `M2 → personal / M2`，策略 failover、已发候选各 1；展开第一列详情为 personal/M1。390×844 时 document.scrollWidth=390。最新资源为 `index-D1WODQFB.js` / `index-DIHmDJnI.css`，类型检查和生产构建通过。

### 安装包构建与资源核对

Windows release daemon SHA256：`F634A6CC80BC915A4AB2FEF684B2C599220BD83B893FF1EAC93A1A438E6687FA`。先放入 `src-tauri/binaries/jev-switch-daemon-x86_64-pc-windows-msvc.exe`，再从 `src-tauri/` 执行 Tauri CLI 2.11.5 构建；该目录约定避免 hook 在 ui/ui 下找 package.json。

首次 NSIS 打包因官方插件下载超时失败。使用 WSL 从日志中的同一 GitHub 官方 release URL 下载 34,304 bytes 的 v0.5.3 DLL（SHA256 `5BA143B5DB4A87D32D6E7802E033330AAE56CBCEABE0D1E3BA41948385AD4709`），备份旧缓存再放入 Tauri 插件缓存。随后正常 CLI 打包通过，没有绕过其文件哈希校验。

最终本地测试安装包（尚未发布，版本仍为现有 0.1.0）：

| 产物 | SHA256 |
|---|---|
| `src-tauri/target/release/bundle/msi/jev-switch_0.1.0_x64_en-US.msi` | `E92CB8F94D200C6EBB1CD17E840AFC9F3C5B0F139FA1BBBBEC6D7C2859F116DE` |
| `src-tauri/target/release/bundle/nsis/jev-switch_0.1.0_x64-setup.exe` | `D61556A1BAA66624D6A74EE1403E3C083738CC99DD5A773D3AFCC72F6F0AF2D9` |

MSI 以 administrative extraction 解包到 `E:/tmp/jev-package-acceptance-20260925-trace`（退出码 0），没有启动应用。随包 daemon 与上述 release 二进制 SHA 相同，JS 为 `7E3DBAAC1675F85EE9F8BFB41F52F3D1F57CC842C2F5F56890FF783804EE80B0`，CSS 为 `04A5EC7C208CA1804F23C0D0C6F608F87F51A69D32519BFDACBFD4196704F3AC`，均与 ui/dist 当前输入相同。

完整冷启动必须释放 11435。此前自动审批拒绝停止旧 Tauri/daemon 的操作后，未绕过该拒绝；本轮已在产物具体可审阅后，通过异步弹窗请求重新尝试的明确授权（或由用户手动退出）。尚未收到答复，不能将安装包构建/解包当作冷启动验收。

### Docker 依赖获取仍未通过

USTC 构建中的 Cargo 在索引阶段约 24 分钟持续零 CPU、无进一步输出，源码快照也早于最新修复；已取消该构建，确认 cargo 进程消失。另用相同官方 Rust 镜像的 curl 查询 index.crates.io/config.json 成功，表明不能笼统宣称所有容器联网不可用。

Dockerfile 将依赖获取拆为有 300 秒上限的 `cargo fetch --locked`，后续编译使用 `--offline`，缓存 registry/target 并将产物复制到稳定阶段路径。`CARGO_HTTP_MULTIPLEXING` 等仅是可选构建参数，默认沿用 Cargo 行为；诊断性关闭 HTTP/2 多路复用没有关闭 TLS 校验。

新的 HTTP/1.1 构建日志 `E:/tmp/jev-ui-acceptance/docker-build-http1.log` 显示 UI 构建通过、APT 复用成功，但 fetch 仍超时（exit 124），构建退出 1。当前无运行中的 Jev Docker 构建或已通过验收的最终镜像。后续应利用可校验的依赖缓存或进一步定位 Cargo 下载路径，不能继续无界等待。

## Docker 构建与持久化实测、仪表盘空态修正（2026-09-25 02:20–02:50）

### 构建恢复与同源资源

从当前 Cargo.lock 选取 212 个 crates.io 包；Windows/WSL 缓存提供了 199 个与锁文件 SHA256 匹配的归档。缺少的 13 个从官方 static.crates.io 补齐并逐个校验，其中一次网络不可达改用 IPv4 后下载成功。仅将这些公开包及对应稀疏索引加入 BuildKit registry 缓存，没有复制个人 Cargo 配置或凭据。

Dockerfile 改为完整缓存优先离线校验，缺失才执行有界联网 fetch；离线依赖检查 2 秒通过，release 编译约 39 秒通过。仍使用 HTTPS、Cargo.lock 和 Cargo 自身包校验。初次成功镜像为 7e811467…；随后仪表盘修复重建为以下检查点：

| 对象 | 当前验收值 |
|---|---|
| 本地镜像 | `jev-switch:acceptance-20260925` |
| 镜像 ID | `sha256:dac95c44fb4bb1c0025ff84c1ca8cc5e00262bc6208ce329f9cbac195dc368e1` |
| Linux daemon SHA256 | `0a36f18a60c1d9f2fd31edbd0e90ffcfce89e943d6ca7fe1cb156c90ed61c6e3` |
| UI JS | `index-B48eZbwp.js`；SHA256 `66932b6b95b252665e1e91fb701ee6bf03167f4474926243b95c036a86ad86b4` |
| UI CSS | `index-DIHmDJnI.css`；SHA256 `04a5ec7c208ca1804f23c0d0c6f608f87f51a69d32519bfdacbfd4196704f3ac` |

日志为 `E:/tmp/jev-ui-acceptance/docker-build-cached.log` 与 `docker-build-dashboard.log`。JS/CSS 在容器实际 HTTP 服务、Windows ui/dist 与 MSI 解包目录中 SHA 一致。daemon 平台不同，二进制 SHA 不应相同；两端由同一批源码/锁文件构建，health 仍为 0.1.0、api_revision=1、build_revision=null，尚未发布。

### 容器功能与重建恢复

隔离 Compose 项目 `jev-acceptance-20260925` 使用当前仓库 Compose 配置的副本，仅调整测试镜像名、容器名、数据目录、宿主回环端口 11437 和重启策略，追加一个不发布端口的受控上游。数据目录 `E:/tmp/jev-docker-acceptance-20260925/data`；随机测试凭据只保存在该临时项目，不进入仓库。

第一次后台测试期间 WSL 整体重启，两测试容器退出 255，其他已有容器也重新启动；网关日志无应用错误。之后保持本次 Compose 前台会话，完成以下检查，没有修改 WSL 系统设置：

- `/health` 放行；公开模型接口无 Token 为 401，有调用 Token 为 200。
- 管理接口对匿名/readonly 返回 401，管理员密码会话与 admin Token 通过；管理员密码会话不能直接充当公开调用 Token。
- 动态添加 `fixture-personal`，创建 `DockerM`，真实请求到独立上游容器并返回固定 noul=0.73；该值是夹具标记，不代表模型质量。
- 入口改名为 `DockerM2` 后旧名 404；禁用后新名 404。调用 Token 停用后 401，恢复后可用。
- 撤销首次从环境导入的 readonly Token；强制重建网关容器后仍为 401，没有因为原环境变量仍在而复活。
- 重建后旧管理员会话失效；提供商、改名后的禁用入口、调用 Token、两条调用记录及用量仍保留。重新启用入口后调用成功。
- 更新至上述最新 UI 镜像并再次重建后，调用仍成功；累计四次，费用未知、错误率 0，最新 HTTP 静态资源 SHA 匹配。容器 HEALTHCHECK 为 healthy。
- Windows 浏览器真实打开 11437，页脚与资源地址同源，显示最新 `index-B48eZbwp.js`。

API 结果保存在临时项目 `result-before.json`、`result-after.json`、`result-current.json`。Docker 测试请求的旧名/禁用入口在路由前拒绝，未计入已执行上游的调用记录；上述四次是成功进入执行的请求。

### 模式切换：功能通过，响应等待仍待修复

实际由 cloud 切到 local，监听从 `0.0.0.0:11435` 改为 `127.0.0.1:11435`：容器内回环免 Token 可读模型，宿主发布端口不能继续提供 local 访问。随后从容器内回环切回 cloud，外部 health 恢复。结果保存为 `result-modes.json`。

但是首次 5 秒客户端超时；用 15 秒预算后成功，并观察到每次切换都出现约 10 秒后的 `serve drain timeout — force abort`。源码证据：`admin::put_mode` 等待 `ListenHandle::rebind`，后者等待旧 `Serving::stop`；旧服务排空又包含当前切换请求。结果依赖 DRAIN_TIMEOUT 才释放响应，不符合流畅操作目标。

下一步须把停止接收新连接与已有连接的排空分开处理，并以真实 TCP 回归证明：切换及时返回、旧监听关闭、新监听可用、慢请求不中断、失败绑定恢复旧地址。修复前不能把“模式最终正确”写成整项性能验收通过。当前测试容器已恢复 cloud；11435 旧 Windows 实例未触碰。

### 仪表盘与补充布局证据

Docker 未登录页面实际将配置读取 401 显示为“0/0 可达、暂无提供商、0 条路由”。已改为权限/加载失败提示，数量保留未知，支持重试；身份变化会重拉配置。探测接口自身失败只记未知，不假称已证明上游不可达；`off` 与 `disabled` 翻译键不一致一并修正。

11436 浏览器复验：未登录显示明确权限提示与未知数量；管理员登录后恢复两提供商、两条路由。11437 最新容器页面也显示相同修正。类型检查与构建通过；此前最新版路径展示的 lint/17 项前端回归通过，本次无新增协议或队列逻辑。

Token 面板补测中英文 390×844，文档宽 390、输入/按钮无横向越界；760×480 的主滚动区 370px，键盘可聚焦创建按钮且按钮位于 y=250–286。仅输入并清空未提交的测试名称，没有创建或修改 Token。选中既有 readonly Token 可读到四次、平均 179ms 的真实统计。已恢复中文和默认视口。

### Windows 安装包更新

仪表盘修复后重新打包 MSI/NSIS。当前本地文件仍名为 0.1.0（测试产物）：

- MSI SHA256：`3E88E3FF037DFF4F328E0A56B3999D4C66572B0D183C453FE345E9A7B191B3DB`。
- NSIS SHA256：`F45E76CE7AB74CA12899012BFF5BCF1E3DA93C0F61EC5B13E51D148DA9B4E792`。
- 当前解包根：`E:/tmp/jev-package-acceptance-20260925-dashboard/PFiles/jev-switch`。daemon SHA 仍为 `F634A6CC80BC915A4AB2FEF684B2C599220BD83B893FF1EAC93A1A438E6687FA`；UI SHA 与上表相同。
- MSI administrative extraction 首次因 TARGETDIR 使用正斜杠被 Windows Installer 视为网络位置（1606/1603）失败；改为原生反斜杠路径后退出 0，三份资源 SHA 比对通过。没有启动应用或执行普通安装。

完整 Tauri 冷启动/退出依然等待此前弹窗的明确回复。自动审批曾拒绝停止旧 Tauri 和 11435 daemon，理由仅为 `blocked by policy`；未重试这些进程操作。原生窗口几何测试和安装包资源核对不能替代该项验收。

## 监听热切及时响应与同版本产物复验（2026-09-25）

### 修复原因与连接生命周期

真实 Docker 请求暴露的等待关系是：

```text
旧链路：切换请求 -> rebind -> 等旧服务全部请求结束
           ^                          |
           +------ 其中包括自己 ------+
                          |
                      10 秒超时

新链路：切换请求 -> 关闭旧 listener 并收到确认
                -> 新 listener 就绪 -> 回复成功
                   |
旧已接受连接 ------+-> 独立排空；最多 10 秒后取消残留连接
```

`listen.rs` 将旧 listener 的关闭确认与已接受连接的排空分开处理。保持 Axum Router、Hyper 传输及每连接的真实 `ConnectInfo(peer)`；同端口需要先释放旧 listener 时也不等待切换请求结束。`JoinSet` 持有连接任务，排空期限到达后取消并回收，包括未结束的 SSE。supervisor 保留正在排空的服务任务，并在整体停机时等待清理。

不重叠地址继续先试绑，失败不关闭旧监听。同端口交接失败会重绑旧地址；若外部进程同时抢占旧地址，明确返回恢复失败，不能承诺这个竞争下仍正常服务。

### 测试证据

- 先用受控上游闸门保持一条请求在途，对旧实现运行 2 秒 rebind 预算，稳定得到 `Elapsed`；证明回归能够捕捉实际等待问题。
- 修复后真实 TCP 测试共 10 项通过，包含本机/LAN 鉴权、及时换端口、旧端口关闭、在途调用继续完成、同端口 local/cloud 交接、重叠绑定失败恢复与 SSE 超时释放。
- 重叠绑定失败用例最初使用另一具体 IP 上的占用端口，Windows 允许相关通配绑定组合，未触发预期失败；改成先确认不可绑定的未配置测试地址 `192.0.2.1`，从通配地址进入交接分支，实际收到 500 且旧监听、health 和配置恢复。没有把不成立的测试夹具当作产品故障。
- `cargo test --offline --locked --manifest-path rs/Cargo.toml --workspace` 在 9 项 TCP 用例检查点通过；补齐第 10 项后，`cargo test --offline --locked --manifest-path rs/Cargo.toml --workspace --features ts-rs` 全套通过，其中 daemon lib 116、core 60、TCP 10。后续仅启动日志调整，再执行 daemon lib 71 项全部通过。
- ts-rs 前后 57 份生成文件 SHA 一致，未改动协议类型。新增直接依赖 `hyper-util` 使用已有锁定版本，没有升级依赖版本。

### Windows 和容器实际运行

Windows `339DDB1D…` release 内核在独立的 11436 实例启动，数据仍来自 `E:/tmp/jev-access-acceptance/data-cloud`。实际测试将端口改为 11438 后恢复，PUT 分别为 **31.905ms / 30.052ms**；每次旧端口均不能再建立连接。测试在临时端口执行 M1，固定夹具值 0.73 返回，其他 readonly Token 的累计调用从 4 增至 5。

两提供商 personal/team、两启用入口 M1/M2 及托管 Token 从持久存储恢复；未带调用 Token 仍为 401，readonly 不能访问管理接口，admin Token 可管理。11436 浏览器刷新后要求重新登录，登录后显示真实两提供商/两路由、Cloud 模式及“实时”事件流。报告保存于 `E:/tmp/jev-access-acceptance/result-rebind-339ddb1d.json`，其中只有摘要，无凭据。

Docker 镜像 `sha256:ed755c5bc43c64dae8265846bcf8bc8aadf4fc9feada86140c220ae3c0788337`，Linux daemon SHA256 `d77d24fac094f6540d9d6d61eabc15d7948ee85a3da6a90038452ecd253ca217`。使用已有隔离数据重建后，cloud→local / local→cloud 请求分别为 **5.204ms / 4.769ms**，3 秒客户端预算内完成；容器回环免 Token、宿主发布端口在 local 下不可达，恢复 cloud 后 health 正常。没有再出现原来的等待排空超时日志。`result-modes-rebind.json` 保存时序；当时累计五次调用、费用未知、错误率 0，`result-current.json` 随后续复验更新为最新检查点。

这些是本次机器与夹具条件下的观测值，不是生产延迟承诺，也不是上游推理性能。

### MSI 资源与主程序核对

热切修复检查点重新执行 Tauri CLI 2.11.5 的 MSI/NSIS bundle，均成功。MSI 用原生反斜杠 TARGETDIR 做 administrative extraction，退出 0，没有执行安装或启动完整应用。

| 检查点产物 | SHA256 |
|---|---|
| Windows daemon / MSI 随包 daemon | `339DDB1D2C1259E4D29C14A766305C4711DB41A345739FB77BAFA0532592EE7D` |
| MSI | `7043FB9C01BC346E1F0C645CB530A78C5057AF234D3B517695B91365818AADEF` |
| NSIS | `09B21034A8F402EC7F23136A4C6B3E886BC0C77AD93DAC158B628C75F4969A93` |
| release Tauri 主程序 | `75C576B5A0ACD0CE45DB1067D5820AAD551A2D39FE3ECA957DAB81666B3E25E4` |
| MSI 随包 Tauri 主程序 | `194BA674D93F25185DA6EBD21AF5A7DEF3A5A2E2DA4C561D7B9D701EE6DAD525` |

解包目录为 `E:/tmp/jev-package-acceptance-20260925-rebind/PFiles/jev-switch`。JS/CSS 仍为 B48eZbwp / DIHmDJnI，与前节完整 SHA 相同。

主程序哈希不同已进一步定位：整个文件只相差偏移 `0x3dfcb8` 起的三个字节，Tauri 将 `__TAURI_BUNDLE_TYPE_VAR_UNK` 替换为 `__TAURI_BUNDLE_TYPE_VAR_MSI`。替换后整份字节完全一致，所有代码段一致；本机依赖 `tauri-utils` 的 `platform.rs` 定义了该打包类型标记。这一格式处理不能被误判为旧壳混入。

### 启动日志追加修正

11436 恢复的托管 Token 已可正常鉴权，但旧启动日志只计算配置文件中的 `auth_tokens`，错误宣称所有 `/v1` 调用将被拒绝。`AuthState` 现在将该数字明确命名为 `legacy_config_tokens`，`build_state` 在迁移完成后报告托管 Token 总数/启用数；只有 cloud 且确无启用 Token 才提示创建调用 Token。日志不输出凭据。这是诊断文案与计数修正，不改变请求权限。

### 最终产物与环境状态（03:25）

上述哈希和时序继续作为已经取得的热切证据。追加启动日志修正后的最终本地测试产物如下，版本仍为 0.1.0，未发布：

| 对象 | SHA256 / 镜像 ID |
|---|---|
| Windows daemon | `3C8C2D7168379E95D4C7C34B4A6CEED53BACEAACB90B1D9144E89CC1DA7E18F6` |
| Docker 镜像 | `sha256:1cef14d0b117be4890c54957ed05416896819c72fbebedb4b015a43157dc11a5` |
| Linux daemon | `ed620ffbb4b7efc98ad72698ef160e6ebc1788417d38bea5e8274e51e4c5bac5` |
| MSI | `B43AF38BB6448E006D075BF0E6624A54C359CF5CB4144D78651E239AB2418502` |
| NSIS | `5BA0C06532E0DBDEF2C127C99176AA1284A9E340B4732CC206548EB0192CB9FD` |

Windows 11436 的当前进程 PID 82904，路径 `E:/tmp/jev-access-acceptance/bin/jev-switch-3c8c2d71.exe`。启动日志正确显示 `legacy_config_tokens=0`、`enabled_tokens=3`、`total_tokens=5`。未认证公开查询 401，托管调用 Token 查询 200，admin Token 查询管理状态 200；原 readonly 的五次调用仍保留。浏览器重新登录后两提供商/两路由及实时流恢复。全局活动是内存 EventBus，重启后为空；个人 SQLite 历史与统计仍保留，两者不混同。

Docker 隔离容器更新至上述镜像后 healthy，日志显示 `legacy_config_tokens=1`、`enabled_tokens=2`、`total_tokens=3`。已撤销的旧环境 Token 没有因重建恢复；当前受控调用成功，累计六次，费用未知、错误率 0。`E:/tmp/jev-docker-acceptance-20260925/result-current.json` 是这一最终运行检查点；构建日志 `E:/tmp/jev-ui-acceptance/docker-build-token-log.log`。

最后一次 MSI administrative extraction 退出 1603，日志明确为 C 盘可用 0 KB，Installer 还需 6,164 KB；PowerShell 同样读到 C 盘 Free=0，E 盘仍有约 157GB。没有把空间不足归因于应用，也未删除用户其他文件。失败日志保留为 `E:/tmp/jev-package-acceptance-20260925-token-log.log`。

使用已经安装的 WiX `dark.exe`，仅将当前进程 TEMP/TMP 指向 E 盘，从 MSI 提取文件到 `E:/tmp/jev-package-acceptance-20260925-token-log-wix`，退出 0。工具另报告若干安装 UI ControlEvent 外键的 DARK1059 反编译警告；没有据此宣称安装 UI 通过或失败。按 File 表映射还原五份文件到 `E:/tmp/jev-package-acceptance-20260925-token-log/PFiles/jev-switch`，主程序经上述格式标记归一后完全匹配，daemon、index.html、JS、CSS 逐字节匹配输入，结果在该目录上两级的 `resource-check.json`。

当前剩余验收条件：释放 C 盘安装器所需空间；收到此前旧 Tauri/11435 停止操作的明确重试授权，或用户手动退出后，完成包内 sidecar 冷启动、复用和退出验证；继续记录真实多显示器 DPI 范围。随后才推进 P6 正式截图、提交和发布。旧完整 Tauri PID 36136 与旧 11435 daemon PID 73496 本轮均未停止。

## 完整图校验、历史保留与产品验收收口（2026-09-25 04:15）

### 配置更新与数据库升级

入口 POST/PUT、整表路由写入与显式 TOML 导入共享完整图校验。空 ID、自环、跨节点环、未知目标，以及重命名造成的无效引用均返回 400；数据库事务回滚后元数据、完整路由、内存 Registry 和实际调用保持原状态。提供商仍被引用时不能删除，停用则保留配置引用。删除服务入口会逐层清理因此失去出边的别名引用，保留其他有效分支；以中间别名调用入口 DELETE 返回 404。

`endpoints_runtime` 的新增用例先在空 ID 创建上复现旧实现错误返回 200，随后覆盖这些拒绝/回滚分支及有效分支保留，六项入口集成用例全部通过。

旧预发布版本曾把 schema v3 和“根入口导入”共用版本号：新库可能漏导入，旧库可能跳过 Token schema 后启动失败。005 改用独立的命名数据迁移表，并按实际表/列修复旧 Token schema。旧入口导入标记仍表示已经执行，避免复活用户删除的入口；首次导入与标记写入在同一 savepoint 中提交。两个迁移回归覆盖新库导入/删除不复活、旧 v3 修复/策略启停/已有日志保留。

006 取消 `call_logs.endpoint_id` 对可删除入口行的级联外键，保留全部日志字段、Token 外键、索引和事件 ID。重建时额外保存旧自增序列高水位：即使表为空，旧序列为 900 时下一个 ID 仍为 901。此前 daemon 的默认连接没有启用外键，不能误报所有旧实例已经丢历史；此次消除了对这个默认值的依赖。

另用受控上游闸门复现了实际漏记：请求已进入上游，删除入口，再放行结果，旧实现返回成功但 Token 请求数为 0。`record_request` 现在使用受理时已核准的入口 ID，不再以完成时入口仍存在为条件。四项历史回归在开启外键后覆盖删除/重启续读、v5 全字段迁移、两种序列高水位、在途调用完成后归属；新请求仍按删除后的状态返回 404。

### 浏览器失败恢复、加载与双语

在 11436 的 `3C8C2D71…` 内核上执行 M1/M2 横比，临时停用 M2 使其返回 404，M1 成功结果保留。将 state 从 `compare-retry-before` 改成 `compare-retry-edited`，恢复 M2 后只重试该列：上游实际收到原快照，M1 没有再次调用，原 M1 结果未变化。M2 已恢复启用。两条实际请求及断言保存在 `E:/tmp/jev-access-acceptance/comparison-retry-evidence.json`，不把夹具 0.73 当模型质量证据。

临时 18769 代理延迟真实列表响应 3 秒，并可返回受控 503，验证 Token 与活动首次加载提示、刷新期间保留内容、失败时保留旧内容并显示错误；SSE 暂时不可用时可通过轮询显示活动。修复了当前 Token 不变时刷新不重取用量的问题：一次实际 M1 调用后，点击刷新，所选 Token 请求数由 4 变 5。加载证据对应 `CLAVacTB`，用量刷新对应 `DXcWnhHg`；摘要在 `E:/tmp/jev-loading-acceptance/evidence.json`。代理与临时标签已关闭，未保留额外测试监听。

最终 UI `index-ZCEhu_bP.js` 补齐活动路由表格的中英文表头、枚举可读名称、逐行输入/删除标签；底层协议值不变。实际从 Routing → DAG → 表格切换语言，中文显示“起点/匹配规则/会话粘性/失败时”，英文显示对应完整名称。390×844 英文视口中文档宽 390、表格滚动容器宽 348，约 530px 表格仅在容器内横向滚动，未撑宽页面。此项由旧 11435 内核托管当前仓库 UI 验证，只证明前端双语/布局，不冒充最新版 Windows 内核验收。已恢复中文与默认视口，未提交任何路由编辑。

### 统一测试与制品来源

- `cargo test --offline --locked --manifest-path rs/Cargo.toml --workspace --features ts-rs` 全套通过：daemon lib 116、入口 6、历史 4、HTTP 8、TCP 10、迁移 2；其他 workspace crate 同样通过。57 份生成类型前后 SHA 无变化。日志：`E:/tmp/jev-access-acceptance/workspace-history-tsrs.log`。
- 前端 `npm run lint`、17 项测试与最终生产构建通过。Windows release、Docker build、MSI/NSIS bundle 均成功。构建日志分别为 `build-history-release.log`、`docker-build-history.log`、`bundle-history.log`（前者/后者在 `E:/tmp/jev-access-acceptance`，Docker 日志在 `E:/tmp/jev-ui-acceptance`）。
- MSI 原生 administrative extraction 退出 0，目录 `E:/tmp/jev-package-acceptance-20260925-history/PFiles/jev-switch`。随包 daemon、index.html、JS、CSS 逐字节匹配；主程序仍只因 MSI 格式标记相差三字节。结果为目录上两级的 `resource-check.json`。这不是实际安装或完整应用启动。

| 本检查点产物 | SHA256 / 镜像 ID |
|---|---|
| Windows daemon | `5286865A7483D151C7ABBE8001DDED4DBFD62E620FC0930F7776348694129EBF` |
| Docker 镜像 | `sha256:fa4b4e78cd1804f7e59426fbcdc851d17dbd8db0761961a84834caac1dcadcf4` |
| Linux daemon | `d8c40e870ddb596d53303c04c4f7c9a1c71156fac59eb668c62460376685a9b2` |
| MSI | `4B52F889DD3FE74584675912959E2AF2E1DE8F49925CBE418E244553D1C80A47` |
| NSIS | `FA5D31CFE78E177F7A08F6FCD94FEAC9F261B517B123376696396635C9C14DAD` |
| JS：index-ZCEhu_bP.js | `D03B2AF6B75C178161E8016D563E514DFB68C0551152EE097AC570849F622BFD` |
| CSS：index-DIHmDJnI.css | `04A5EC7C208CA1804F23C0D0C6F608F87F51A69D32519BFDACBFD4196704F3AC` |

Docker 新镜像、MSI 和本地 UI 的 index.html/JS/CSS 哈希一致。所有制品仍是未发布的 0.1.0 测试版本。

### Docker 实际升级与当前运行边界

重新检查时，既有隔离容器已为 exited。第一次 Compose 启动漏传既有 project 名而发生容器名冲突；核对容器标签后使用原 `-p jev-acceptance-20260925` 正常重建并启动，误建的空网络已删除。未改动其他项目容器。当前 11437 镜像为上表 `fa4b4e78…`，running/healthy；Compose 前台会话保持 WSL 测试实例运行。

升级前备份了 SQLite。实际启动后 schema 为 6，原有 6 条 call_logs 每个字段均与备份相同，旧撤销环境 Token 仍为 401，只读 Token 仍不能管理。旧入口实际调用返回夹具 0.73；再创建 `DockerHistoryMigration`、尝试无效引用并附带停用，收到 400 且路由未变，实际调用继续成功。删除该测试入口后新调用 404，但统计、个人事件 ID/detail 完全保持，累计请求数为 8。证据：`E:/tmp/jev-docker-acceptance-20260925/result-history.json`；此前的 `result-current.json` 是累计 7 次的前一检查点。

Windows 切换 11436 测试 daemon 的命令被自动审批拒绝，理由仅为 `blocked by policy`，整条命令未执行。已完成包内资源核对后，单独请求明确重试授权，尚未收到答复；没有将同一拒绝动作拆分重试。11436 仍为 PID 82904 / `3C8C2D71…`，已备份的数据库仍为 schema 4、12 条调用、5 个 Token、M1/M2。新 Windows daemon 仅完成构建和随包核对，未实际切换。

此前旧完整 Tauri PID 36136 与 11435 daemon PID 73496 的停止重试授权也仍待答复，这两项均未停止。P5 保留最新版 Windows 切换、完整 Tauri 冷启动/复用/退出、实际安装和真实多显示器 DPI 验收；P6 正式截图、提交与发布仍在其后。C 盘此时约 528MB 空闲，原生文件提取的空间障碍已消除，不再将“C 盘为零”当作当前事实。

## 容器正常停机与交付门禁（2026-09-25 04:42）

### 重启持久性与发现的停机缺口

对上一检查点的隔离 Docker 实例执行实际重启，八条历史及全部字段、配置、Token 身份/角色/启停、统计、自增序列均保留；已删除入口没有重新发布，已撤销的旧环境 Token 仍被拒绝，个人事件增量游标没有产生重复。核对请求本身会推进有效 Token 的 `last_used_at`，因此该字段检查单调递增，其余相关字段逐项保持不变。结果为 `E:/tmp/jev-docker-acceptance-20260925/result-history-restart.json`。

但 Docker 事件同时揭示另一问题：旧 `fa4b4e78…` 内核只监听 Ctrl-C，没有处理容器发送的 SIGTERM，重启等待 15 秒后以 SIGKILL / exit 137 结束。数据恢复通过不能说明停机过程正常；两项结论必须分开。

Unix 主程序现在同时等待 SIGINT/SIGTERM，收到后记录信号并进入已有 `ListenHandle.shutdown`；非 Unix 保留 Ctrl-C 路径。监听关闭、在途连接排空和 10 秒超时取消继续由已有监听层负责，没有另造第二套停机流程。Compose 的 `stop_grace_period` 设置为 15 秒，为排空与进程退出留出余量。

### Linux 与真实容器验证

- 新增 Unix 子进程测试：独立配置/数据目录及临时端口，等待真实 `/health`，发送 SIGTERM，要求五秒内 exit 0 且日志记录 SIGTERM。Windows 编译通过但运行零项；Linux BuildKit 使用已有依赖缓存、禁用网络实际运行 **1/1 通过，0.55 秒**。日志为 `E:/tmp/jev-docker-acceptance-20260925/signal-test.log`，没有把 Windows 的零项当成信号验证。
- 新镜像中用受控上游暂时保持一条真实调用，执行 `docker stop --timeout 15`。从容器网络空间确认监听约 **562.496ms** 后关闭；此时上游尚未放行，进程仍等待。放行后请求正常返回夹具值 0.73，整个 stop 约 **1644.116ms**，容器 exit **0**。重启后该次调用已持久记录，累计 **9** 次。证据为 `E:/tmp/jev-docker-acceptance-20260925/result-sigterm.json`。
- 上述观测包含测试控制与 Docker 命令开销，不是生产延迟承诺；0.73 是夹具输出，不是模型质量证据。取消连接也不保证取消上游已经执行的计算或费用。
- 随后按 15 秒配置重建同一隔离 Compose 实例，检查为 running/healthy，实际 `Config.StopTimeout=15`。启动时间为 `2026-09-24T20:35:51Z`；04:42 只读 SQLite 核对仍为 9 条日志、自增序列 9。未改动其他项目容器。

### 最新制品与资源一致性

| 本检查点产物 | SHA256 / 镜像 ID |
|---|---|
| Windows daemon | `F8534846040B598E8D791FB5A46E19D3A00166AD5950D4651575B260F020F779` |
| Docker 镜像 | `sha256:02b531c22746238e90d23213218398adfd28abee9cdcab38520ab932af5c89d2` |
| Linux daemon | `c0895b03c04a68d13bb3dab2dbd1066d2afde16f616cbc4346583410ccab9651` |
| MSI | `9EB29A439045908D47F62E0FD33A8C51CAB452C3697795751872FCBF8A396000` |
| NSIS | `B905B8002F02279BEFCD06AEE8EB2ABF1E817EB33C7E9759A888FF11F4AA1363` |

前端未改动，仍为 `index-ZCEhu_bP.js` / `index-DIHmDJnI.css`，完整 SHA 见 04:15 表格。最新运行容器、MSI 和本地构建的 index.html/JS/CSS 哈希相同。

Windows release 与 MSI/NSIS bundle 成功；原生 MSI administrative extraction 退出 0，目录为 `E:/tmp/jev-package-acceptance-20260925-signal/PFiles/jev-switch`。随包 daemon、index.html、JS、CSS 与输入逐字节相同；Tauri 主程序归一 `UNK→MSI` 格式标记后完全一致。结果保存在目录上两级 `resource-check.json`。构建日志为 `E:/tmp/jev-access-acceptance/build-signal-release.log`、`bundle-signal.log` 和 `E:/tmp/jev-ui-acceptance/docker-build-signal.log`。所有制品仍是未发布的 0.1.0 测试版本，未执行安装或完整 Tauri 启动。

### 交付门禁与未完成范围

CI workspace 测试加入 `--locked`，CI 与 Windows release 的 UI 安装统一为 `npm ci`。ts-rs 门禁除已跟踪文件 diff 外，也拒绝生成目录中新出现但未跟踪的文件，避免漏提交新增 DTO。已在独立临时 Git 仓库证明能够检出 `Missing.ts`；两个 workflow 的 YAML 解析、门禁 shell 语法和 diff 检查通过，没有将这些本地检查写成远端 CI 已运行。

[发版手册](../RELEASE.md) 明确四处版本字段和三份锁文件需要同步，使用显式暂存覆盖新增文件；`0.6.0` 仅为命令示例，当前版本仍是 `0.1.0`，没有替用户决定版本。本轮未执行提交、推送、tag、远端 workflow 或发布。

Windows 11436 仍为旧 `3C8C2D71…` 测试内核；旧完整 Tauri/11435 同样未被停止。此前两项运行切换被自动审批返回 `blocked by policy`，具体重试授权尚未答复，没有重复或拆分执行被拒动作。最新 Windows 制品只是构建和资源核对完成，P5 的同版本 Windows、完整 Tauri 冷启动/复用/退出、安装器交互、真实多显示器 DPI 仍待验；P6 正式截图和发布继续在产品验收之后。


## 发布启动指引与版本输入校验（2026-09-25）

复查发现旧发布页只给出 `docker run -p ... image`：没有持久卷，也没有管理员初始化步骤。发布模板和发版手册现提供相同启动路径：先设置管理员密码，挂载命名数据卷，显式使用 cloud 与容器内通配监听，宿主发布到回环，设置 15 秒停机预算，再登录创建调用 Token。发布正文从 Docker job 的输出取得实际小写镜像名，避免构建标签与展示命令不一致。这是发布准备，尚未创建或更新任何远端 Release。

`workflow_dispatch` 的版本输入此前直接插入 Bash 源码；现通过 `REQUESTED_VERSION` 环境变量按普通字符串读取。版本仍需与仓库四处字段一致，输入不会被当成命令执行。使用从 YAML 提取的原始门禁脚本，在 WSL Bash/Node 对当前仓库做六组实际执行：空输入、匹配输入和匹配 tag 通过；版本不一致、tag 不一致、包含 shell 命令替换语法的输入均被拒绝，拒绝时没有写出发布输出或执行输入中的命令。另测混合大小写仓库名转为小写，合计七项通过。

证据为 `E:/tmp/jev-release-gate-20260925/result.json`。两个 workflow YAML 解析、最新发布命令的 Bash 语法及变更文件 diff 检查通过。未拉取/发布镜像，未触发远端 workflow，也未把本地脚本测试当作远端 CI 验收。

## Windows 同版本隔离联调（2026-09-25）

在确认 11439 空闲后，启动独立 cloud 测试实例，明确绑定 `127.0.0.1:11439`，使用 `E:/tmp/jev-windows-isolated-20260925` 内的新配置、数据库和日志。测试只控制自己创建的进程句柄，没有停止、替换或注入旧 Tauri/11435/11436；完成后测试 daemon 和本地 HTTP 夹具全部退出，11439 无监听。

实际执行的 Windows daemon 为 `E:/tmp/jev-access-acceptance/bin/jev-switch-f8534846.exe`，SHA256 与 04:42 表中 `F8534846…` 完全一致。`/health` 返回产品 `jev-switch`、版本 `0.1.0`、`api_revision=1`；`build_revision` 实际为 null，本地构建未注入提交标识，因此本次由不可变路径与完整文件 SHA 确认来源。

同一 daemon 通过 HTTP 提供 index.html、`index-ZCEhu_bP.js` 和 `index-DIHmDJnI.css`，响应均为 200，哈希与最新仓库构建、MSI 和 Docker 内资源一致。此项验证了同源托管来源，没有重新执行一遍未改动的前端视口矩阵。

14 项场景全部通过：二进制及健康身份、静态资源、创建调用 Token、创建入口与路由、实际 HTTP 调用、重启恢复路由、停用后 404、恢复启用、无效环路返回 400 且完整图保持不变、拒绝后原调用仍正常、删除入口后 404、删除后历史保留、再次重启后入口不复活且统计/事件 ID 不变。三次成功调用的归属、累计统计和事件明细均保留。

上游为本次测试创建的本机临时 HTTP 服务，返回的概率 0.91、usage 和费用都是显式夹具值。它证明真实 HTTP 路由与计量传递，不证明真实模型质量、推理性能或云账单。结果为 `E:/tmp/jev-windows-isolated-20260925/result.json`，日志与脚本同目录，结果不含凭据。

该验证补齐最新 Windows 内核的独立 Web 运行证据；新库测试不冒充旧 11436 数据库原地升级，进程重启也不冒充完整 Tauri 的 sidecar 生命周期。此前 Docker 旧库升级、删除历史与 SIGTERM 证据继续分别保留。完整 Tauri、安装器及实际系统 DPI 仍按 P5 自身门槛验收。

## 三屏实机几何与剩余验收门槛（2026-09-25 05:05）

真实 Tauri 隐藏窗口逐屏验证通过：150% 上方横屏、125% 左侧竖屏和 100% 主屏。实际工作区分别为 1920×1128、1080×1860、1600×852；对应外框均完整可见、满足 90% 尺寸预算并保持居中，负坐标没有丢失。新原生测试 1/1 通过，常规壳测试 5 passed / 2 ignored。详细几何、运行命令和边界见[响应式记录](../responsiveness-check-2026-09-24.md#2026-09-25-0505真实三屏与-dpi-几何)。

测试使用独立 profile，在每块屏上主动执行首窗函数，未启动完整应用或 daemon。它补齐真实多屏首窗几何证据，不声称用户跨屏时会自动重设尺寸。只有测试代码改变，生产函数、UI、Windows daemon 与 MSI/NSIS 均仍对应 04:42 的资源表。新窗口测试和 11439 Web 测试进程已退出。

| 验收项 | 当前证据与结论 | 后续动作 |
|---|---|---|
| 最新 Windows Web 内核与同源资源 | F853、ZCE/DIHm；隔离 11439 的 14 项通过 | 本项已取得运行证据；旧 11436 原地替换仍未执行 |
| Docker 部署与持久化 | 升级、重建/重启、历史保留、在途调用与 SIGTERM exit 0 | 已验，沿当前镜像保留证据 |
| 首窗与响应式 | 浏览器视口矩阵、弹窗/长表格、中英操作；真实 100%/125%/150% 三屏几何 | 已验范围明确；完整桌面交互仍随下一项验收 |
| 完整 Tauri 生命周期 | 新壳/sidecar/UI 随包核对通过，完整应用仍是旧实例 | 处理已被拒绝的旧实例停止/替换后，验证冷启动、复用、退出及完整界面操作 |
| 安装器实际行为 | MSI/NSIS 构建、MSI 原生文件提取与资源匹配通过 | 仍需实际安装/启动验证，不能以解包代替 |
| P6 正式展示和发布 | 本地发布脚本七项检查通过，未提交/推送/运行远端 CI | P5 整体验收后统一截图、最终文档、提交与既定发布验证 |

P6 文档收尾还应改正 README 中旧的“三页”“八个端点”“TOML 真值源”“二部图”及固定历史测试数量；这些是已定位的待更新条目，不应直接作为当前产品说明。正式截图仍遵守用户指定顺序。

05:05 再次只读核对：旧完整 Tauri PID 36136、11435 daemon PID 73496、11436 daemon PID 82904 仍存在，路径与此前记录一致；没有发现本次新建的 F853 或窗口测试进程遗留。自动审批拒绝所需的具体重试授权尚无答复。该条件已连续多个目标轮次保留；现有可独立完成的 Windows Web、Docker、三屏几何和发布准备已完成，整版桌面验收及其后的 P6 仍受此外部条件限制。

## 隔离 Tauri 冷启动与 Windows 历史迁移（2026-09-25 08:45）

本节是 05:05 检查点的后续状态，旧段落仅保留为当时的观测，不代表当前进程状态。

### 旧进程清理与冷启动

再次只读确认旧实例来源后，按用户明确要求重试同一停止命令，PID 73496 和 82904 均成功退出；此前旧完整 Tauri PID 36136 已不存在。这是获授权的原命令成功执行，并未绕过自动审批或换路执行。旧 Demo 清理表仍作为先前清理结果保留。

从 MSI 的 administrative extraction 目录启动包内 Tauri 主程序，使用独立测试存储目录：

- 包路径：`E:/tmp/jev-package-acceptance-20260925-signal/PFiles/jev-switch/jev-switch.exe`
- 壳 SHA256：`194BA674D93F25185DA6EBD21AF5A7DEF3A5A2E2DA4C561D7B9D701EE6DAD525`
- sidecar SHA256：`F8534846040B598E8D791FB5A46E19D3A00166AD5950D4651575B260F020F779`
- UI：`index-ZCEhu_bP.js` / `D03B2AF6B75C178161E8016D563E514DFB68C0551152EE097AC570849F622BFD`；`index-DIHmDJnI.css` / `04A5EC7C208CA1804F23C0D0C6F608F87F51A69D32519BFDACBFD4196704F3AC`
- 隔离数据：`E:/tmp/jev-tauri-cold-start-20260925/appdata` 与 `E:/tmp/jev-tauri-cold-start-20260925/localdata`

原生窗口实测为 `x=292, y=82, 1016×689`，完整位于 `1600×852` 工作区内并居中。WebView CSS 视口为 `1000×650`。四个真实页面 Dashboard、Providers、Routing、Playground 均渲染完成，页面宽度均为 1000px，无水平溢出、无 renderer exception。操作系统选择的界面语言为中文。结果文件：`E:/tmp/jev-tauri-cold-start-20260925/native-window.json`、`E:/tmp/jev-tauri-cold-start-20260925/ui-smoke.json`。

health 报告产品 `jev-switch`、版本 `0.1.0`、`api_revision=1`、`build_revision=null`。端口 11435 属于随包 sidecar；WebView 调试端口为 9239。测试期间未发起真实上游请求；配置、数据库和日志全部位于隔离目录。取证完成后只停止本次创建的 Tauri PID 30152 与 sidecar PID 77988；端口 11435 和 9239 均释放。冷启动证明包内壳/sidecar/静态资源能运行，不等同于 MSI 实际安装验证。

### Windows v4→v6 数据升级

从 `E:/tmp/jev-access-acceptance/before-history-migration.db` 创建字节副本，在隔离目录 `E:/tmp/jev-windows-upgrade-20260925/run` 运行最新 Windows sidecar。源备份 SHA256 为 `85AEA7772C4FCA262626B232A214D7551B33B9028F67C9100724F4364937D25F`；只操作副本，配套 provider TOML 也复制到隔离目录。

十项检查全部通过：二进制/health 身份、schema v4→v6、12 条历史记录逐行逐字段一致、13 列与自增序列 12、一次性导入标记不变、provider 两次读取、M1/M2 入口读取、只读用量统计、admin 写入返回 401、重启后入口/历史/schema/统计保持。测试未调用上游；原始数据库备份未修改。机器可读结果为 `E:/tmp/jev-windows-upgrade-20260925/run/result.json`。

### 尚未完成的复用场景与当前边界

在包冷启动完成并退出后，尝试启动一个独立兼容 daemon 来验证 Tauri 复用已有服务的路径。该启动命令被自动审批拦截，工具返回的唯一理由为 `blocked by policy`，命令未执行；复用路径因此仍是未验证项。没有用其他 shell、脚本、可执行文件或启动机制重试同一被拒绝动作。

复查时，旧目标 PID 36136、73496、82904 及本次测试 PID 30152、77988 均不存在；11435、11436、9239、9240 均无监听。隔离 reuse fixture 未创建。P5 仍待：复用已有 daemon、实际 MSI 安装与用户托盘退出路径；本轮冷启动和 v4→v6 迁移分别只覆盖各自描述的范围。P6 最新截图、README 校正、提交/发布验证仍须等 P5 产品验收收口。

## NSIS 当前用户安装与实装联调（2026-09-25 09:13）

### 安装、来源与运行

为覆盖不要求管理员权限的真实安装路径，按包内 NSIS 配置 `INSTALLMODE=currentUser` 执行 `src-tauri/target/release/bundle/nsis/jev-switch_0.1.0_x64-setup.exe /S /D=E:/tmp/jev-nsis-install-20260925/app`。安装进程退出码 0；HKCU 卸载登记为 `jev-switch 0.1.0`，安装目录和卸载器路径与隔离目标相符。

安装后核对：

| 文件 | 安装后 SHA256 | 与构建输入 |
|---|---|---|
| `jev-switch-daemon.exe` | `F8534846040B598E8D791FB5A46E19D3A00166AD5950D4651575B260F020F779` | 完全相同 |
| `ui/dist/assets/index-ZCEhu_bP.js` | `D03B2AF6B75C178161E8016D563E514DFB68C0551152EE097AC570849F622BFD` | 完全相同 |
| `ui/dist/assets/index-DIHmDJnI.css` | `04A5EC7C208CA1804F23C0D0C6F608F87F51A69D32519BFDACBFD4196704F3AC` | 完全相同 |
| `jev-switch.exe` | `3AC4D59E425F91432D60CA8B20E0A5074AE459128807E95E1F923CEF3154DD58` | 与 release 主程序长度相同，仅 3 字节不同 |

启动的是安装目录中的 `jev-switch.exe`（PID 79900），sidecar PID 34584、WebView2 PID 53660。`/health` 返回产品 `jev-switch`、版本 `0.1.0`、`api_revision=1`，`build_revision=null`。sidecar 只监听 `127.0.0.1:11435`。本地 UI CDP 走查 Dashboard、Providers、Routing、Playground 四页，均为 CSS 视口 `1000×650`、文档宽 1000px、无横向溢出；renderer exception 计数为 0。原生窗口位于 `(292,82)`，外框 `1016×689`，工作区 `1600×852`、96 DPI，完整居中可见。

daemon 配置、SQLite 与日志通过 `APPDATA` 环境变量隔离在 `E:/tmp/jev-nsis-install-20260925/appdata/jev-switch`。该库 `call_logs=0`，未调用真实上游；用户现有 `C:/Users/56506/AppData/Roaming/jev-switch/providers.toml` 和 `C:/Users/56506/.jev-switch` 未被此次进程触碰。WebView2 自身仍使用默认 profile `C:/Users/56506/AppData/Local/io.github.arcj137442.jevswitch/EBWebView`；该 profile 早于本轮存在，实测后保留、未删除。页面与完整安装证据见 `E:/tmp/jev-nsis-install-20260925/ui-smoke.json` 和 `install-evidence.json`。

### 卸载、MSI 与仍未覆盖的动作

测试结束后终止的仅为本次启动的 Tauri、sidecar 与 WebView2 进程。NSIS 当前用户卸载器退出码 0，安装目录及 HKCU 卸载登记消失，11435/9241 均释放；E 盘隔离数据和证据文件保留。

同一批 MSI 的实际安装尚未完成。它在 WiX 包中声明 `InstallScope="perMachine"`；当前执行令牌未提升，C 盘当时约 0.47GB 可用。尝试用 Windows 标准 `RunAs` 启动 MSI 后，约 90 秒没有对应 `msiexec.exe`、安装日志、文件或注册表项；随后仅结束了本次等待中的 PowerShell 包装进程。没有把 NSIS 通过当作 MSI 通过，也没有换其他路径重试提权安装。

本次也未交互操作系统托盘菜单；“退出”处理在源码中会设置 quitting 标记、调用 `app.exit(0)`，并由 `RunEvent::Exit` 关闭 sidecar，但源码路径不能替代真实托盘操作验收。P5 仍需 Tauri 复用已有 daemon、MSI 安装以及实际托盘退出。README 校正、正式截图和发布门禁仍依 P6 顺序保留。

真实模型调用条件另行只读核验：两份现有 provider TOML 都没有明文 key；`AI_GATEWAY_API_KEY` 未出现在当前进程、用户或系统环境中；配置的本机 Laya `127.0.0.1:18765` 未监听。没有输出任何凭据值，也没有发起真实模型请求。待有效上游凭据或本地模型服务就绪后，仍需用受限输入完成真实调用/失败路径，并分别核对实际 route trace 与调用计量。

## 真实上游调用前置条件复核（2026-09-25 09:33）

对 `%APPDATA%\jev-switch\providers.toml` 与 `%USERPROFILE%\.jev-switch\providers.toml` 只做安全元数据检查，不读取或输出任何密钥值。两份 TOML 均无明文 key，Vercel 配置只引用 `AI_GATEWAY_API_KEY`；该变量不在当前进程、用户环境或系统环境中。Laya 地址为 `127.0.0.1:18765`，TCP 检查无监听。因此本轮未对任何外部模型发请求；要完成 P5 的真实模型/失败路径验收，必须先有真实 Vercel 凭据或实际运行的本地模型服务。旧 provider 文件只读检查，未修改。

## 运行时身份探测 TCP 回归（2026-09-25 09:45）

在 `src-tauri/src/runtime_probe.rs` 增加一个临时 loopback TCP 测试：测试服务绑定系统分配的 `127.0.0.1:0` 端口，收到请求后核对首行是 `GET /health HTTP/1.1`，再返回版本与 API revision 匹配的 Jev health JSON。实际 `probe()` 结果为 `RuntimeProbe::Ready`，服务线程正常结束。未使用产品端口，没有启动真实 daemon，也没有修改应用配置或用户数据。

验证结果：`runtime_probe::tests` 3 项通过；完整 `cargo test --manifest-path src-tauri/Cargo.toml --locked --offline` 为 6 项通过、2 项忽略、0 项失败。两项忽略测试需要交互式 Windows 原生 WebView 环境。此测试验证身份探测函数的 TCP/HTTP 交换；完整 Tauri 进程面对另一进程的既有 daemon 并实际走复用分支，仍未验证。既有被拦截的启动操作没有通过其他工具或启动机制重试。MSI per-machine 安装、托盘菜单退出和真实上游请求也仍待各自的实际条件与验收。

## 当前工作树回归复核（2026-09-25 09:54）

对当前源码重新运行默认 workspace 与 CI 中启用 `ts-rs` 的测试命令：默认组 203 项通过；`ts-rs` 组 260 项通过。58 个 `ui/src/generated` 与 `rs/crates/jev-protocol/bindings` 文件在 `ts-rs` 测试前后的 SHA256 完全相同。`ui` 的 `npm test` 为 17/17 通过，`npm run build` 的 TypeScript 检查和 Vite 生产构建通过，资源名仍为 `index-ZCEhu_bP.js` / `index-DIHmDJnI.css`。Tauri shell 测试为 6 passed、2 ignored；忽略项需要交互式原生 WebView 环境。

质量检查边界：`cargo fmt --all -- --check` 对工作树中的多份 Rust 源码/测试报告格式差异，未通过；CI 当前没有配置 rustfmt 门禁。运行 `ts-rs` 时，生成器仍对部分 serde alias 打出解析警告，但绑定文件没有发生漂移。以上两项都单独记录，不把它们冒充产品验收通过，也没有对整个工作树执行格式化以免重写并行改动。

## 追加 Tauri 包内调用联调的工具门槛（2026-09-25 10:11）

准备以提取目录中经 SHA256 核对的 Tauri 包，使用全新 E 盘 APPDATA、WebView profile 与本机固定响应上游，补测入口创建、调用、停用和恢复。工具在 PowerShell 命令执行前返回 `blocked by policy`；因此夹具文件/目录、进程和监听均未创建。随后只读核对确认 `E:/tmp/jev-tauri-local-integration-20260925` 不存在、没有 Jev 可执行进程，11435、18768、9239 仍无监听。当前工具列表没有 Tauri MCP；本轮未通过 Node、其他 shell 或进程接口重试这个启动动作。它是额外尝试的包内冷启动联调，失败不影响此前已有的 MSI 解包冷启动与 NSIS currentUser 实装证据；但也不补足完整 Tauri 交互、复用既有 daemon、托盘退出、MSI per-machine 安装或真实上游调用。

## MSI 机器级实装与已安装 Tauri 启动（2026-09-25 10:23）

用户确认先前批准的 UAC 属于 Jev Switch 安装，并已完成安装、看到 Tauri 界面启动。随后只读核验确认：

- 安装源是 `src-tauri/target/release/bundle/msi/jev-switch_0.1.0_x64_en-US.msi`，SHA256 `9EB29A439045908D47F62E0FD33A8C51CAB452C3697795751872FCBF8A396000`。MSI ProductName/ProductVersion 为 `jev-switch 0.1.0`，`ALLUSERS=1`；verbose MSI 日志记录安装成功、错误状态 0。
- HKLM Windows Installer 卸载登记为 `jev-switch 0.1.0`，ProductCode `{36C89DAC-A2D0-4E54-A7FA-673C04767998}`，安装目录为 `E:/tmp/jev-nsis-install-20260925/app`。当前 `jev-switch.exe` 与 `jev-switch-daemon.exe` 均由该目录运行；daemon 仅监听 `127.0.0.1:11435`。
- 已安装 Tauri sidecar 的 `/health` 返回 `status=ok`、产品 `jev-switch`、版本 `0.1.0`、`api_revision=1`、`build_revision=null`。用户确认桌面界面正常出现；本轮没有取得新截图或该次原生窗口尺寸数据。
- 安装后的主程序 SHA256 为 `194BA674D93F25185DA6EBD21AF5A7DEF3A5A2E2DA4C561D7B9D701EE6DAD525`；daemon 为 `F8534846040B598E8D791FB5A46E19D3A00166AD5950D4651575B260F020F779`；JS/CSS 为 `D03B2AF6B75C178161E8016D563E514DFB68C0551152EE097AC570849F622BFD` / `04A5EC7C208CA1804F23C0D0C6F608F87F51A69D32519BFDACBFD4196704F3AC`。这些与本次 MSI 随包内容相符，JS/CSS 也与当前 `ui/dist` 相符。
- 本次安装使用默认 `%APPDATA%\jev-switch`。应用运行配置含 Vercel 地址 `https://ai-gateway.vercel.sh/v4/ai/evaluation-model`，但未解析到 API key；runtime snapshot、当前 shell 及用户/机器环境均未提供 `AI_GATEWAY_API_KEY`。本地核查先前测试 TOML/JSON、相关 SQLite 数据库及指定 Claude Opus 5 会话记录，没有找到真实 Vercel key。可见的具体 key 只属于本地 Laya 夹具、`example.invalid` 自动化测试或 loopback demo，均不导入；密钥值没有写入终端结果或文档。
- 已安装版 `jev-switch.db` 在 10:24 留有两条失败调用：`jev`、`jev-zh` 均为 HTTP 503，耗时约 6.7 秒，记录未选出 upstream provider/model。没有把失败记录误判为真实上游验收，也没有继续发起无凭据请求。成功调用仍需在取得真实 Vercel key 后进行。

该检查点完成了 MSI perMachine 实装、实际安装版启动、daemon 身份与随包资源来源核对。它不替代复用已有 daemon、托盘退出、安装版窗口/DPI 几何、真实 Vercel 请求和 P6 展示/发布验收。MSI verbose 日志在 `E:/tmp/jev-switch-msi-install.log`，含 Windows 用户名元数据，没有 API key；删除该临时日志的操作被执行策略拦截，因此没有换其他方式删除，日志未进入仓库。

## 已安装 Tauri 的 Vercel 配置与连通探测（2026-09-25 11:12）

本次请求进一步核对并从已安装实例执行了只读诊断。`GET http://127.0.0.1:11435/health` 确认 `jev-switch 0.1.0`、API revision 1；已安装 runtime 中有 2 个启用 provider（`laya`、`vercel`），Vercel base URL 为 `https://ai-gateway.vercel.sh/v4/ai/evaluation-model`，公开服务入口 `jev-vercel` 已存在。当前路由为 `jev-vercel → typesafe-ai/jev → vercel`（provider route priority 0，入口策略 `follow_global`）。管理 API 返回 `api_key_set=false`；运行时 SQLite 与当前 shell、用户、机器环境均无 `AI_GATEWAY_API_KEY`。

通过该安装版自己的 `POST /v1/admin/providers/vercel/probe` 发起了探测；sidecar 对配置的 Vercel URL 执行免鉴权 GET，收到 HTTP 405，往返 766 ms，probe 标记 `ok=true`。这只证明网络能到达服务端且该路径不接受 GET，不证明 key 有效、路由可调用或模型返回成功。没有发起模型请求或产生模型调用费用。由于在既有资料/当前机器上仍未找到真实 key，本次没有伪造配置、导入夹具 key 或写入任何 secret。安装版真实模型调用仍待 key 安全录入后由同一 Tauri 实例完成。

## 已安装 Tauri 窗口可见状态复核（2026-09-25 11:33）

对已安装主进程 PID 1620 做只读窗口句柄枚举，没有观察到可见的应用尺寸顶层窗口：枚举到一个不可见的 1200×615 窗口和若干 16×16/零尺寸辅助窗口。`Get-Process.MainWindowHandle` 单独返回的 13×13 矩形位于屏幕外，不作为尺寸证据。当前源码的初始窗口配置为 1000×650、最小 760×480；此前隔离 MSI 冷启动记录的外框为 1016×689、CSS 视口 1000×650。由于本次已安装实例没有可见主窗，不能将隐藏句柄尺寸记作它当前的显示几何，也不能据此确认用户首次打开后的窗口尺寸；需要在应用主窗恢复显示后再测。此次未向窗口发送输入或关闭应用。

## 已安装 Tauri 的桌面恢复尝试（2026-09-25 11:41）

按 `computer-use` 技能列出的 Windows app 清单选中唯一运行的 `jev-switch` 窗口（路径 `E:/tmp/jev-nsis-install-20260925/app/jev-switch.exe`，标题 `Jev-Switch`），调用窗口激活时两次均返回 `failed to activate captured window`；中间重新枚举并重新绑定窗口后再次失败。根据技能的恢复规则停止桌面输入，没有沿用坐标/句柄，也没有切换到 PowerShell/UIA 等自动化方式。没有获得新截图或 key 字段界面证据。窗口仍需在桌面端恢复显示；真实 Vercel key 仍未设置。

## 已安装实例的壳/sidecar 进程关系（2026-09-25 11:47）

只读进程快照显示安装目录中的 `jev-switch.exe` PID 1620 于 10:23:04 启动；`jev-switch-daemon.exe` PID 62236 在 10:23:05 启动，Windows 父 PID 是 1620，且两者位于同一安装目录。此证据表明当前这次运行由 Tauri 壳启动 sidecar，不能作为“启动时复用已存在 daemon”的通过证据。没有停止或重启该进程。

## Tauri 启动分支回归测试（2026-09-25 12:00）

将 `sidecar::start()` 的探测结果分派抽到生产路径实际调用的 `dispatch_startup_probe()`，并加入三项测试：兼容 daemon 只走复用回调、不启动子进程；不兼容端口返回错误且不复用/覆盖；端口不可用时才走 spawn 回调并透传 spawn 错误。`cargo test --manifest-path src-tauri/Cargo.toml --locked --offline` 通过（9 passed、0 failed、2 ignored）；忽略项需要交互式 Windows WebView。`rustfmt --edition 2021 --check src-tauri/src/sidecar.rs` 通过。

此回归覆盖壳的分派逻辑，不证明已安装 Tauri 实际面对另一个已运行的兼容 daemon 时成功复用；当前安装实例的父子进程证据仍显示由壳启动 sidecar。完整桌面复用验收保持待测。

## 紧凑首窗默认值与已安装 Vercel 状态复核（2026-09-25 12:28）

### 首窗尺寸

用户反馈初次打开的 Tauri 窗口仍偏大。此前在 1600×852 主屏实测的内容区是 1000×650、外框 1016×689，约占工作区高度 81%。因此将源码默认逻辑尺寸和 `window_layout` 上限改为 900×560，常规最小 760×480、90% 工作区预算、DPI 换算与居中逻辑保留。配置默认值也一起修改，避免显示器信息读取失败时回退到旧的大尺寸。

`native_initial_window_stays_inside_current_work_area` 使用独立 WebView2 profile，实际隐藏原生窗口测得：主屏 1600×852、96 DPI，内容 900×560，外框 916×599，位置 `(342,127)`，完整居中且可见。新增紧凑默认值单测后，`cargo test --manifest-path src-tauri/Cargo.toml --locked --offline` 为 10 passed、0 failed、2 ignored；两个 ignored 测试需显式运行原生 Windows 窗口环境。`rustfmt --edition 2021 --check src-tauri/src/window_layout.rs src-tauri/src/main.rs` 通过。

该结果证明本地源码构建下首窗算法和原生尺寸；没有渲染四页，没有打包或更新当前 MSI 安装版。已安装 Tauri 仍是既有 0.1.0 运行实例，需等新包冷启动后核实完整页面与首次窗口体验。现有 CSS 响应式记录有 760×480、1000×650、1280×400、手机及宽屏矩阵，尚未为新的 900×560 组合重新截取四页截图。

### 安装版 Vercel 配置

从当前运行的安装版 `127.0.0.1:11435` 只读核验：health 为 `jev-switch 0.1.0`、local 模式；Vercel provider/base 已配置且启用，`api_key_set=false`；`jev-vercel` 入口启用，路由链是 `jev-vercel → typesafe-ai/jev → vercel`。本轮脚本在 Opus5 会话和 712 个候选 provider/config 文件中定向查找目标上游凭据，没有发现 Vercel key；确认到的 keyed fixture 指向 localhost 或 `example.invalid`，均未导入。进程、用户和机器范围的 `AI_GATEWAY_API_KEY` 均不存在。

安装版自身的 `POST /v1/admin/providers/vercel/probe` 返回 `ok=true`、HTTP 405、486 ms；含义仍仅是 GET 到达了远端，不证明 key 或模型成功。没有对模型发请求、没有写入密钥，也没有修改当前安装版配置。真实调用等待用户在已打开的应用 Providers 页录入 key，或提供其本机文件路径/环境变量名；不得把密钥写进会话、日志或计划文件。
## 隔离 Tauri 壳复用当前安装版 daemon（2026-09-25 12:44）

先确认 `127.0.0.1:11435/health` 返回匹配的 Jev 身份、版本和 API revision，再启动一个隐藏的隔离 Tauri WebView 测试壳。该壳调用生产路径共用的 `start_with_probe()` 和 `spawn_watch()`：探测到的健康响应来自当前 MSI 安装版 daemon；启动逻辑命中复用分支后，WebView 从既有 daemon 成功导航到 `http://127.0.0.1:11435/`。

为避免服务中途退出时产生副作用，测试注入的 spawn 回调只记录并返回错误，不会创建子进程。断言通过：导航加载成功、spawn 回调未执行、ShellState 没有子进程；测试壳以 exit 0 退出后再次探测，原 daemon 仍为 `RuntimeProbe::Ready`。WebView2 数据目录隔离在 `E:/tmp/jev-tauri-reuse-live-<test-process-id>`，未修改现有应用数据、停止或替换当前安装进程。

执行命令：

```powershell
$env:JEV_TAURI_REUSE_TEST_DIR = 'E:\tmp\jev-tauri-reuse-live-<test-process-id>'
cargo test --manifest-path src-tauri/Cargo.toml --locked --offline isolated_tauri_shell_reuses_live_daemon_and_leaves_it_running -- --ignored --nocapture
```

结果为 **1 passed**；随后完整 Tauri 壳测试 **10 passed、3 ignored、0 failed**，定向 `rustfmt --check` 通过。该测试证明生产身份探测、复用分支、等待线程和真实本地 UI 导航组合可与运行中的安装版 daemon 协作；测试壳未注册主程序的单实例插件和托盘，也不是已安装主程序本身的二次启动。因此安装版完整冷启动/复用/托盘退出仍须保留在 P5。

## 900×560 当前源码包构建与 MSI 资源核对（2026-09-25 13:02）

将收紧后的 900×560 初始窗口尺寸编入 Windows 发布包，构建使用离线锁定依赖：`cargo build --release --manifest-path rs/Cargo.toml --locked --offline -p jev-switch-daemon`、`npm run build --prefix ui` 与 `cargo tauri build` 均通过；前端 `npm test --prefix ui` 为 17/17 通过，Tauri 壳测试为 10 passed、3 ignored，另有与当前安装版 daemon 联调的复用测试 1/1 通过。本轮补跑 `cargo test --manifest-path rs/Cargo.toml --workspace --locked --offline`，workspace 测试通过，退出码 0。

当前构建产物的 SHA256：

| 产物 | SHA256 |
| --- | --- |
| Tauri 主程序 `jev-switch.exe` | `1D2B0ED5D41AE62E1C7D16FBB80D34F9A23FD11F484654231D956ECDF34BD590` |
| 随包 daemon | `1A9C88BB09090726307571325718121359E159C19A19D3041C4036A044C4FDC4` |
| MSI | `5AEAD90863D611609B799B167576EAE46B016FF03CBADB1A61ADF557A86732C8` |
| NSIS | `CE50090D609DD090E8F33890ACA0EE4F61E17C6D0D2D9CE90CAD1BF3E1B593E1` |

用 WiX `dark.exe` 提取 MSI 后，核对主程序、daemon、HTML、JS 与 CSS 的哈希均匹配本次构建输入；没有将安装器替换到现有用户实例，也没有启动新包或触碰该实例的数据目录。用户当前看到的机器级安装仍是旧 MSI（主程序 `194BA674…`、daemon `F8534846…`）；新包所含 UI bundle 的 JS/CSS 与已安装实例相同，但新首窗几何和整体安装包冷启动还未在新包中验证。因此本检查点只证明构建与资源封装一致，P5 仍待新包冷启动与完整页面验收。

同一时点只读复核运行中的已安装 sidecar：Vercel provider 启用，base 为 `https://ai-gateway.vercel.sh/v4/ai/evaluation-model`，`api_key_set=false`；路由仍为 `jev-vercel → typesafe-ai/jev → vercel`。未找到可导入的既有真实 key，因此本轮没有写 provider 配置，也没有发送真实模型请求。免鉴权 probe 的 HTTP 405 仍只代表远端路径可达，不代表鉴权或模型调用成功。真实调用须待 key 由本机安全录入后，再经安装版完成。

## 已安装 Tauri 导入真实凭据与实际调用（2026-09-25 13:54）

当前机器级安装进程仍为 `E:\tmp\jev-nsis-install-20260925\app\jev-switch.exe`（PID 1620）及其 daemon（PID 62236，`127.0.0.1:11435`）；主窗口标题为 `Jev-Switch`。本次没有停止、替换或重启该实例。

用户提供的真实 Vercel 凭据通过该已安装 daemon 的 `PUT /v1/admin/providers` 写入现有 `vercel` provider。写入时保留完整提供商表，核验到 provider 数量仍为 2、`laya` 保留、Vercel 启用且地址未变，管理 API 返回 `api_key_set=true`。`/v1/admin/config/storage` 显示运行配置权威为 SQLite、TOML 快照无漂移。凭据明文没有写入本仓库文档、命令输出或测试日志。

随后向本机安装版 `POST /v1/systemone` 发送一条合成判断请求，目标公开入口为 `jev-vercel`，当前配置链为 `jev-vercel → typesafe-ai/jev → vercel`。daemon 的事件和 SQLite 调用记录确认该请求已进入决策处理并记为 **HTTP 404 / 847 ms**；记录没有成功选出的 provider/model、回答或 usage。该次测试因此未通过真实模型调用验收；现有证据也不足以单独断定 404 是由模型、上游路径还是上游路由返回。

对照 Vercel 官方文档（[TypeSafe API](https://vercel.com/docs/ai-gateway/sdks-and-apis/typesafe)，页面标注 2026-09-21 更新；[Evaluation HTTP API](https://vercel.com/docs/ai-gateway/modalities/evaluation)，页面标注 2026-09-22 更新），当前文档列出的 Jev 兼容地址为 `https://ai-gateway.vercel.sh/typesafe/v1/systemone`；通用 evaluation HTTP API 为 `https://ai-gateway.vercel.sh/v1/evaluate`。仓库 adapter 与已安装配置仍使用 `/v4/ai/evaluation-model`、`ai-model-id` 头和旧 body 形状。此协议差异是 404 的优先排查方向，但在新协议得到成功响应前不记作已证明根因。

下一步应先为当前 TypeSafe API 增加明确的 adapter 请求/响应回归测试，并更新示例配置；随后构建隔离候选验证请求体、路由 trace 与真实响应。安装版凭据已保存，但旧安装实例暂时保持运行且没有改其 base URL；在候选通过前，不覆盖该实例。P5 的真实 Vercel 调用仍未通过。

## TypeSafe adapter 修正与候选 daemon 实网核验（2026-09-25 14:22）

依据 Vercel 当前 TypeSafe HTTP 文档，将源码中的 Vercel transport 改为向 `/typesafe/v1/systemone` 发送原样 Jev `{model,state,questions}`，通过 Bearer key 鉴权；不再在活动请求路径里把 `noul` 改成 `boolean`。响应解析保留 Jev probability、usage 与 provider metadata；默认 TOML 与 capability 文档同步更新。新增 wiremock 回归，检查路径、Authorization、完整 Jev 请求体与 usage 解析。

离线 `cargo test --manifest-path rs/Cargo.toml --workspace --locked --offline` 全部通过。随后以新构建 daemon、独立配置目录和 `127.0.0.1:11439` 做了一次真实上游调用；key 由子进程环境变量提供，没有写入候选配置。`typesafe-ai/jev` 返回 HTTP 200，route trace 选中 provider `vercel` / model `typesafe-ai/jev`，Jev `noul` 概率为 0.99，usage 为 352 input / 23 output tokens，1 次 upstream，daemon 记录延迟 31,596 ms；费用字段仍为 unknown。该实测证明**当前源码构建的 daemon** 已能按官方 TypeSafe API 完成一条真实调用，不代表当前已安装 Tauri 已升级或验证成功。Choice/Score 缺少 confidence 的响应映射仍需单独做协议核验。

当前机器级安装 Tauri 仍保持原样并继续运行；其中已保存的 Vercel key 与旧 `/v4/ai/evaluation-model` 配置未改。之前对这个已安装实例的真实调用仍是 HTTP 404 / 847 ms，不能计作通过。源码候选的 Vercel provider 已通过临时管理 API 禁用，但测试 daemon PID 70548 仍监听 11439；key 仍在该进程继承的环境块中，直到该进程结束。尝试停止该临时候选并删除隔离目录的操作被自动策略拒绝（`blocked by policy`），没有继续尝试绕过；需要用户从任务管理器结束 PID 70548 后，再清理 `E:\tmp\jev-vercel-typesafe-candidate-20260925`。P5 仍待新包/已安装 Tauri 上的实测、Choice/Score 协议确认及其余验收。

## TypeSafe adapter 更新后的 Tauri 包构建（2026-09-25 14:34）

按 `docs/deployment.md` 从 `src-tauri` 目录构建，避免在仓库根目录执行时把 `beforeBuildCommand` 的 `ui` 相对路径解析为 `ui/ui`。先用最新 release daemon 刷新 Tauri external sidecar，再运行 `cargo tauri build`；Tauri 自动完成 UI TypeScript 检查与 Vite 生产构建，NSIS/MSI 均成功生成。新的 NSIS SHA256 为 `D9BE4CAE577B0F93E8CE38F87D04BB79F357C22DA6173F738CCF8DEC165B64A4`，MSI SHA256 为 `DF5CB194676757161E1994F7F371E9F367F9B9EEA088D7BF65B13DE9255703BB`。此前 UI tests 为 17/17，Rust workspace 离线测试全通过，`git diff --check` 通过（仅换行符提示）。

这两个是尚未安装的新候选包。机器级旧安装版仍运行于 `E:\tmp\jev-nsis-install-20260925\app`，Tauri PID 1620、daemon PID 62236 持续监听 11435；源码候选 daemon PID 70548 仍监听 11439，调用后 Vercel 已禁用，但它的进程环境仍保留 API key。此前对候选进程清理动作收到 `blocked by policy` 后，没有再换方法停止进程。新包尚未安装、冷启动或验证 UI/daemon 同版本一致。

## TypeSafe Choice/Score 候选核验、首启文件权限修复与同 revision 制品（2026-09-25 15:53）

### 11439 上已记录的真实 Vercel 调用

只读读取仍运行的 `127.0.0.1:11439/v1/admin/events?since=0&limit=500`，不读取事件之外的请求正文或凭据。两条已持久请求记录如下：

| 请求 | 成功 | 上游 | 耗时 | Usage | upstream calls | 候选事件 `cost_usd` |
|---|---:|---|---:|---|---:|---|
| Noul | HTTP 200 | `vercel / typesafe-ai/jev` | 31,596 ms | 352 input / 23 output tokens | 1 | 未记录 |
| Choice + Score | HTTP 200 | `vercel / typesafe-ai/jev` | 1,034 ms | 418 input / 51 output tokens | 1 | 未记录 |

组合调用的原始响应为 Choice `billing`（confidence 1.0）和 Score `1.98`（confidence 0.97）。provider metadata 带 `gatewayCost=0` 与 `marketCost=0.000017556`，但目前事件的 `cost_usd` 仍为空；这是当前候选进程的版本边界。当前源码已经加入只读取 `providerMetadata.gateway.gatewayCost`、拒绝负值/非有限值、不推估成本的解析逻辑，相关 wiremock 和 Choice/Score confidence 回归通过。11439 当前 health `build_revision=null`，Vercel provider 已 disabled、`api_key_set=true`，TypeSafe base 未变；这说明它不是本次带 cost parser 的最终桌面构建，不能用其日志证明新解析已在 live runtime 生效。

### Rust 与前端门禁

在去掉 `auth.rs` cloud middleware 中无用 pattern 绑定后，Rust workspace + `ts-rs` feature 以离线锁定依赖跑完，命令退出 0；ts-rs 对若干现存 `serde(alias)` 标注有 warning，但测试与类型导出完成。前端 `npm run lint --prefix ui` 通过；`npm test --prefix ui` 17/17；`npm run build --prefix ui` 通过，产物仍为 `index-ZCEhu_bP.js` 和 `index-DIHmDJnI.css`。适配器回归覆盖 TypeSafe Jev 路径与请求体、Choice/Score confidence、usage 和 gateway cost；没有将市场价折算成 `cost_usd`。

### Docker 首启权限及重启实测

首个 smoke 在全新具名卷中观察到 `providers.toml` mode 755，并出现密钥文件权限告警。Dockerfile 现仅在首次播种时对新复制的 `providers.toml` 执行 `chmod 600`；已存在的数据文件不会被这段首启分支更改。用最终镜像重做隔离测试，确认首启文件 mode 600 且无该权限告警。

镜像 `jev-switch:typesafe-confidence-cost-final-20260925` digest 为 `sha256:7ffc508548c72a4714440d117c0457e541d3da1673f746cf162effd9f21a8aef`，内嵌 build revision `typesafe-confidence-cost-perms-20260925`。全新数据卷里的真实容器测试确认：`/health` identity/revision 匹配；cloud 无 Authorization 的 `/v1/models` 为 401；有效只读 Token 为 200；管理员密码能换发 session；`/` 返回托管 UI HTML；配置权威为 SQLite。执行 container restart 后重复 admin login，SQLite `current_fingerprint` 保持 `1b4eb04a5f345d9a`，调用 Token 仍可用，状态码 200。测试只使用无效于任何上游的专用 acceptance 凭据，未向 Vercel 发请求；专用容器与 volume 已停止并删除，没有触及 `lingmou-*` 或其他既有容器/数据。

### Windows MSI/NSIS 构建与安装器门槛

为让 Windows sidecar 与 Docker health 可比，daemon release 以同一 `JEV_BUILD_REVISION=typesafe-confidence-cost-perms-20260925` 编译，revision 字符串嵌入二进制。release daemon 与 `src-tauri/binaries/jev-switch-daemon-x86_64-pc-windows-msvc.exe` 的 SHA256 均为 `E071CB195E7963E2F40F7CFF85C9EB148477CF13A0BD9FD78AB022CA1951264A`。`cargo tauri build` 从 `src-tauri` 执行成功；Tauri 主程序 SHA256 `0B38C9947C76FE56DDFBC5B30131F8C68EEC049FB8382FC5AF01C05175677530`，MSI `06AAF402F0E880554E3E72905F551E7FC16B9DB4D014B7406E286E153468137F`，NSIS `57E1F591AE7EE551ABD253CFAD1C0A9864BAC87558E4D6D16E75B86C16984857`。随包构建输入 JS/CSS SHA256 分别为 `D03B2AF6B75C178161E8016D563E514DFB68C0551152EE097AC570849F622BFD`、`04A5EC7C208CA1804F23C0D0C6F608F87F51A69D32519BFDACBFD4196704F3AC`。

Tauri CLI 生成 MSI/NSIS 时两次报告 bundle-type 标记写入失败，并提示 updater package 可能不能识别类型；本项目目前未依赖 `tauri-plugin-updater`，构建本身退出 0。尝试 `msiexec /a` 两次均返回 1603：verbose log 明确指出 Windows Installer 在 C 盘需要 6,176 KB、可用空间为 0；即使 TEMP/TMP 与目标都在 E 盘也不能通过 `InstallValidate`。机器级安装没有被该动作修改，临时诊断日志已删除。实际安装前 C 盘仍须释放 Installer 所需空间。

### MSI 内嵌 payload 只读提取复核（2026-09-25 16:17）

不运行 Installer 动作，使用只读 Windows Installer 数据库 API 从当前 MSI 的 `_Streams` 表读取 `app.cab`（6,000,491 bytes，SHA256 `4C110247D9828B291CCB31E0FD44201EC46BD06F904ABDE8C7D231BECE23F41F`），再以系统 `expand.exe` 展开到 `E:/tmp/jev-msi-unpacked-typesafe-final-20260925`。实际解出五个文件，shell、daemon、`index.html`、JS 与 CSS 均逐字节匹配本轮构建输入；分别匹配 `0B38C994…75677530`、`E071CB19…C63F8DEA`、`1D58352C…7761D0B`、`D03B2AF6…F622BFD`、`04A5EC7C…6704F3AC`。本项证明最新 MSI 封装来源正确；没有启动 shell 或修改当前安装。下一项仍是腾出 C 盘空间后做真实安装/冷启动。

### 仍需用户协助释放的已安装运行时

最新机器快照仍显示旧安装 Tauri PID 1620 与其 daemon PID 62236 占用 11435；已安装 Vercel provider enabled、`api_key_set=true`，但 base 仍是 `/v4/ai/evaluation-model`。候选 PID 70548 占用 11439，候选 provider 已 disabled 但该进程环境仍保留已授权的 key。新 MSI/NSIS 尚未安装或启动，新 Tauri 的首窗 900×560、多页渲染、真实 Vercel `noul/choice/score`、新版 gatewayCost 日志、主程序复用现有 daemon、托盘退出、保存/禁用/重启恢复尚未实测。此前结束旧候选 PID 的动作收到自动审批 `blocked by policy`；没有通过另一种 shell、UI、安装程序或 API 重做同一终止动作。继续安装与真实桌面验收前，需要用户退出当前 Jev Switch，在任务管理器结束 PID 70548，并在 C 盘释放 Installer 要求的约 6.2 MB；之后本任务再核验端口释放、安装最新包、保留数据并完成冷启动联调。

## P5 增量：5173 实际状态与新旧前后端分离（2026-09-25 16:33）

检查时 `127.0.0.1:5173` 无监听，已有 Edge 标签虽指向该 URL，实际打开会得到 `ERR_CONNECTION_REFUSED`。没有触碰已有标签；本轮通过 `web-access` CDP 新建隔离标签，并从当前工作树启动 Vite 5.4.21 到 5173。Dashboard、Providers、Routing、Playground 均加载当前源码；Playground 可见公开入口与直连上游目标及“运行全部/取消全部”操作，Routing 显示入口配置和 DAG 导航。1592×768 CSS 视口下四页 document `scrollWidth=1584`，没有水平溢出。该视口结果不代表系统 DPI 或原生 Tauri 几何。

前端 `getBase()` 在 Vite 开发模式默认请求 `127.0.0.1:11435`。Providers 与调用历史显示的是机器级安装旧 daemon 的数据；Vercel base 仍是旧 `/v4/ai/evaluation-model`。经浏览器读取 11435、11439 的 `/health`，两者都是 Jev Switch 0.1.0 / API revision 1 / `build_revision=null`。因此本次证明的是当前前端源能呈现四页和新入口/比较 UI；由于内核不匹配，不能作为同 revision 联调通过。只读取页面状态，没有提交、探测或模型调用，配置未改，密钥字段仍掩码显示。本轮创建的标签与 Vite 进程已关闭，5173 已恢复无监听；安装版和候选 daemon 仍在运行，C 盘可用空间为 0 GB。

## P5 增量：同 revision 的隔离 Web 实跑（2026-09-25 16:45）

之后把本轮生成的空白本地配置放在 `E:/tmp/jev-switch-same-revision-web-20260925-1633/providers.toml`，用 Windows sidecar 在 `127.0.0.1:11440` 启动；配置无 provider、无 key、无真实用户数据。其 `/health` 为 `typesafe-confidence-cost-perms-20260925`。当前工作树 Vite 再次运行于 5173，并只在本轮新建的浏览器标签内把 `window.__JEV_BASE__` 指到 11440。浏览器从 `http://127.0.0.1:5173` 跨源 `GET /health` 返回 200；四页正确显示该隔离实例的运行状态和空配置态，1592×768 下无水平溢出。没有写入设置、创建 Token、探测 provider 或运行模型，因此这次证明同 revision Web 页面能连接新内核并读取状态，不证明写入/重启恢复或真实上游路径。

测试完成后关闭了新建标签、Vite 与 sidecar，5173/11440 均无监听。无凭据隔离配置、SQLite 与启动日志保留在 E 盘临时测试目录供复核；原有安装 PID 1620/62236 与旧候选 PID 70548 保持运行且未修改。

## P5 增量：最新版 Tauri 壳并行启动边界（2026-09-25 16:55）

源码核对发现，Tauri Builder 首先注册 `tauri-plugin-single-instance`；shell 的运行时探测、等待页目标和 `UI_ORIGIN` 均固定在 `127.0.0.1:11435`。据此判断，在机器级旧 shell PID 1620 仍运行时直接启动同 identifier 的新包会触发单实例转交；若只避开单实例，shell 仍会探测并复用旧 daemon PID 62236。两种情形都不能证明新 MSI 自身的 sidecar 冷启动，因此本轮没有启动第二个同产品壳，也没有修改产品 identifier 或硬编码端口来制造隔离假象。这条是基于实现的边界判断，未宣称已完成 Tauri 运行验收。

16:55 只读复核：11435 由 PID 62236 监听，11439 由 PID 70548 监听；5173、11440 无监听。下一步仍需退出机器级旧应用、结束候选进程并为 MSI 在 C 盘腾出空间，再安装/启动已核对 payload 的最新版 MSI，检查配置保留、真实上游、四页布局及托盘生命周期。

## P5 自动化生命周期覆盖复核（2026-09-25 17:07）

针对隔离 Web 运行时的后续验证，先检查现有自动化是否已有相同覆盖：`endpoints_runtime::endpoint_routes_are_persisted_replaced_and_disabled_at_runtime` 覆盖创建/调用/停用与停用后不触达 provider；`persisted_endpoint_route_is_restored_by_new_build_state` 覆盖新的 state 从 SQLite 恢复路由并实际调度；`endpoint_and_global_strategy_settings_drive_real_scheduler_paths` 覆盖策略；`history_retention::endpoint_delete_keeps_token_history_with_foreign_keys_enabled_and_after_restart` 覆盖调用计量与删除/重建后的历史保留。其详细断言位于 [endpoint runtime tests](../../rs/crates/jev-switch-daemon/tests/endpoints_runtime.rs) 与 [history retention tests](../../rs/crates/jev-switch-daemon/tests/history_retention.rs)。这些均为 fake adapter 的隔离 Rust HTTP 测试，不能替代 OS 子进程、Tauri 壳、真实上游或实际安装验收。

## P5 增量：安装器要求管理员权限（2026-09-25 20:32）

用户交接报告已记录旧 Jev 进程全部退出、相关端口释放及 C 盘 L1 清理完成。随后只读复核 C: 有 758.4 MB 可用，Jev 进程不存在，11435/11439 无监听。WebView2 仍有进程，但分别属于 Clash Verge、CC Switch 与 GameViewer，其 `--user-data-dir` 均不是 Jev profile；未终止这些进程。

使用报告中已核对的最新 MSI，以普通权限运行 `msiexec /i <MSI> /qn /l*v E:\tmp\jev-msi-install-final-20260925.log`。安装器退出 1603；日志在 `InstallInitialize` 明确报告 Error 1730：“You must be an Administrator to remove this application.” 日志并显示静默安装禁用了 elevation prompt。失败后旧卸载登记 `{36C89DAC-A2D0-4E54-A7FA-673C04767998}` 仍在，未观察到 Jev 进程或端口重新出现；因此此动作没有证明新版已安装。

随后通过标准 `Start-Process -Verb RunAs` 启动交互式安装，没有改用替代 shell 或禁用权限检查。系统产生 UAC `consent.exe`（PID 39188，20:26:42）；20:32 复核时该进程仍在，提权日志尚未生成。UAC 窗口须由用户亲自核对处理；Agent 不操作安全确认界面。等待用户处理后再查看 MSI 详细日志、ProductCode/安装路径、运行时 `/health` 与四页表现。

## P5 增量：新版已安装启动、真实 TypeSafe 调用与包内 UI 联调（2026-09-25 21:42）

### 安装、启动和运行身份

用户已完成 UAC 并安装。本轮读取 `E:\tmp\jev-msi-install-elevated-20260925.log`，确认安装退出状态 0 且末尾为 `Product: jev-switch -- Installation completed successfully`。机器级 ProductCode 为 `{408A33CD-E2A4-481F-8CCE-F6C9777C3554}`，安装目录为 `E:\tmp\jev-nsis-install-20260925\app\`。随后启动已安装 `jev-switch.exe`，PID 11680，主窗口标题 `Jev-Switch`、Responding=True；其 daemon 子进程 PID 84236 监听 `127.0.0.1:11435`。

已安装 shell SHA256 为 `0B38C9947C76FE56DDFBC5B30131F8C68EEC049FB8382FC5AF01C05175677530`，sidecar SHA256 为 `E071CB195E7963E2F40F7CFF85C9EB148477CF13A0BD9FD78AB022CA1951264A`。`GET /health` 返回 HTTP 200、`status=ok`、`api_revision=1`、`build_revision=typesafe-confidence-cost-perms-20260925`；本次测试确认实际进程即目标 revision。

### 保留凭据并迁移旧 Vercel 配置

升级后用户 APPDATA 中的配置与数据库仍在。只调用管理 API 的 `api_key_set` 状态验证凭据存在，不读取、打印或复制 API key。旧 Vercel base 为 `https://ai-gateway.vercel.sh/v4/ai/evaluation-model`；按照本版提供商更新语义，PUT 时只更新 base、略去 key 字段来保留原 key。更新后的 base 为 `https://ai-gateway.vercel.sh/typesafe/v1/systemone`，回读确认 Vercel 仍 enabled 且 `api_key_set=true`。

更新 base 后第一次对公开 model `jev-vercel` 的真实调用返回 404。daemon 日志与路由管理读回表明调用把外部 ID `jev-vercel` 传到了 Vercel，原因是保存路由 `jev-vercel → typesafe-ai/jev` 缺少 `upstream_model`。将该边补为 `upstream_model=typesafe-ai/jev`；API 回读保持 9 条路由，priority 10 未变，daemon 热更新后请求重新走 Vercel TypeSafe adapter。404 事件保留在历史中。

### 已安装 sidecar 的真实模型结果

通过 `127.0.0.1:11435/v1/systemone` 对新版已安装 daemon 做了两次真实调用。第一条 Noul 为 HTTP 200，选中 `vercel / typesafe-ai/jev`，usage `311 input / 20 output`，1166 ms，1 upstream call。第二条 Choice+Score 组合也为 HTTP 200，Choice=`billing`、confidence 1.0，Score=2.38、confidence 0.44，usage `400 input / 46 output`，597 ms，1 upstream call。管理事件回读均为 `cost_usd=0.0`；provider metadata 的 `gatewayCost=0` 单独成立，另有 `marketCost` 数值但不将其冒充网关收费。用例为合成客服输入，未在文档或命令输出中暴露凭据。

管理事件还保留初始失败记录：HTTP 404、858 ms、未选定 provider。它证明旧 alias 映射配置会导致真实上游失败，也证明补入 model rewrite 后 live route 已成功；不要从两条最新成功事件推断失败路径没有发生。

### 包内静态 UI 对已安装 daemon 的四页检查

为不覆盖安装文件，将安装包 `ui/dist` 复制到 `E:\tmp\jev-switch-ui-review-20260925`，仅在副本 `index.html` 加入 `window.__JEV_BASE__="http://127.0.0.1:11435"`。临时静态服务在 `127.0.0.1:5173` 运行，Codex IAB 隔离标签读取这份包内 JS/CSS，并直接读取已安装 sidecar：

- Dashboard 显示 daemon 正常、Local 模式、9 条路由，最近调用列出两条成功和一条旧 404。
- Providers 显示 2/2 启用；只确认 Vercel key 已配置，UI 未揭露 key。
- Routing 的 DAG 显示 9 条边；重点边为 `jev-vercel → typesafe-ai/jev / typesafe-ai/jev`，提供商端显示新 TypeSafe URL。
- Playground 显示共享输入表单、公开入口与直连上游的目标选择区；已选 `jev` 和 `jev-vercel` 两个入口，可“运行全部”。没有在此浏览器副本额外提交模型请求或修改配置。
- 1280×720 IAB 截图显示输入/目标区域使用主内容宽度，未出现历史描述的左侧大块空白。截图来自浏览器，不是 Tauri WebView；尚不能据此判定安装版原生窗口在 900×560、其他长宽比、Windows DPI 或多屏环境下正确。

临时静态服务和 IAB 标签已关闭；没有更改包内安装文件。尝试删除测试副本时，自动审批拦截了针对 `E:\tmp\jev-switch-ui-review-20260925` 的递归删除命令，因此该无凭据副本保留在 E 盘，仅含包内静态资源和 localhost API base 覆盖，不含 API key；没有换另一条删除路径重试。`apps=[]` 且当前 CUA 未提供 Windows 原生 window API，因此只能只读确认原生进程有窗口且响应，不能读取原生 Tauri screenshot、托盘菜单或实际客户区像素。本轮未操作托盘或宣称完成原生窗口验收。原生 Tauri 四页实景、首启大小、DPI/多屏、托盘退出与退出重启后的 DB 状态仍为 P5 未完成项。P5 的安装包身份、真实 TypeSafe 调用和配置热更证据已补齐；P6 仍未开始。

## P5 增量：本地 Laya 实路由与请求追踪（2026-09-26 00:42）

### Laya 服务与网关配置

复用已经运行的本地 Laya 服务，避免在 RTX 2080 Ti 上重复加载模型：PID 79984，仓库启动脚本 `scripts/laya_multi_server.py --port 18767 --device cuda`，监听 `127.0.0.1:18767`。`/health` 返回 `mode=multi-model`、`device=cuda`，`english` 与 `multilingual` 均已加载。已安装 Jev sidecar PID 84236 监听 `127.0.0.1:11435`，`/health` 返回 `status=ok` 和预期 build revision。

安装版 Laya provider 原先配置为无人监听的 18765 端口。本轮将其 API base 更新为 `http://127.0.0.1:18767/v1/systemone`，回读确认启用。Vercel provider 保持启用且 TypeSafe base 不变；更新 Laya 时未包含 Vercel key 字段，回读只确认 `api_key_set=true`，不读取或记录凭据内容。

### 从公开模型入口到 Laya 的真实请求

通过已安装网关调用公开接口，管理事件能把客户端模型 ID、选中入口、提供商、上游模型、状态、计量和路由 trace 关联起来。事件 ID 是该本机数据库内的序号。

| 事件 | 对外请求模型 | 选中 provider / 上游模型 | 结果 | 路由证据 |
|---|---|---|---|---|
| 4 | `laya-english` | `laya / laya-english` | HTTP 200，77 input tokens，436 ms | trace 选中 `laya`，priority 0；`x_backend_model=english`，Noul 返回 0.8556 |
| 5 | `jev-zh` | `laya / laya-multilingual` | HTTP 200，95 input tokens，141 ms | trace 选择 `laya-multilingual`，priority 20；`x_backend_model=multilingual`，中文 Choice=`billing`，confidence 0.5191 |
| 6 | Playground 的 `jev-zh` | `laya / laya-multilingual` | HTTP 200，136 input tokens，117 ms | 公开入口请求进入网关事件历史，确认 Playground 也走过同一条路由 |
| 7 | `jev` | `laya / laya-english` | HTTP 200，88 input tokens，214 ms | 此次测试临时禁用 Vercel 后仅有 Laya adapter 可选；trace 选中 Laya priority 30，`x_backend_model=english`，答案为 logistics、confidence 0.9277 |
| 8 | `laya-english`（新鲜复测，2026-09-26 00:54） | `laya / laya-english` | HTTP 200，46 input tokens，205 ms | 完整响应 trace：requested/selected=`laya-english`，hop/provider=`laya`，priority 0，1 次上游调用，gateway latency 205 ms；Noul 返回 0.3461 |

事件 7 的 Vercel 临时禁用由 `try/finally` 包围，完成后立即回读确认 Vercel 与 Laya 都恢复 enabled。由于 daemon 只把启用的 provider 装入 adapter registry，此次 `jev` 请求只可能到达 Laya，没有向 Vercel 发出调用。最终 provider 状态为 Laya 指向 18767、Vercel 指向现有 TypeSafe endpoint 且 key 仍已配置。Laya 的网关成本没有可靠数值，按未知记录，不解释成零。

事件 8 是本轮重新发起的直接复测。请求实时响应含完整 `route_trace`；安装版管理事件 ID 8 持久化记录 `endpoint_id=laya-english`、provider/upstream model、HTTP 200、46 input tokens、205 ms 与一次上游调用，cost 保持未知。当前管理事件/call log 没有持久化完整 per-hop trace，也没有 request ID 将完整响应和历史行严格绑定；本次可用唯一的 endpoint/provider/model/status/时间记录交叉复核，但长期审计仍应增加 trace 存档和稳定关联 ID。

### Playground 与前端可追溯性

在包内静态 UI 的 Playground 中，用中文工单分类样例并排运行两个本地目标：公开入口 `jev-zh` 与直连 `laya · laya-multilingual`。两张卡均完成，结果均为“物流”；公开入口卡展示完整入口/路由/策略，直连卡展示 provider、模型和客户端耗时。只有公开入口请求进入网关事件历史；直连 provider 预览不经过公开入口调度，所以它的结果保留在 UI 卡片中，而不会伪称存在一条公开路由事件。

这验证了本地模型服务、Jev adapter、公开模型路由、返回结果及管理事件之间的端到端链路。它不代表已经覆盖真实 Tauri WebView 的页面截图、托盘操作或关闭重启后的完整 DB 生命周期；这些仍属 P5 待验收项。

## 2026-09-26 01:25：调用历史结构化摘要与当前 Tauri 状态

用户指出调用历史把 JSON 作为主要响应内容，普通使用者难以快速理解。本地事件 `detail` 本身已经包含入口 ID、provider、上游模型、HTTP 状态、耗时、上游调用数、Token usage 与成本；问题在前端把整个 JSON 字符串直接渲染。`ui/src/components/access/AccessDashboard.tsx` 现在先展示这些可读字段，将原始 JSON 收入默认折叠区；未知字段、坏数据与其他事件仍可按需展开原文。新解析器校验不可信 JSON 值，并为窄屏隐藏次要表格列。

`ui/tests/activity-detail.test.mjs` 覆盖正常 Laya 事件、零值与 HTTP 失败推断、坏 JSON/非 request 事件、错误字段类型，共 4 项；本轮 `npm test` 为 21 passed、0 failed，`npm run build` 的 TypeScript 和 Vite 生产构建通过。浏览器源 UI 曾连接已安装 daemon 并显示本机历史事件 8 的可读摘要和折叠 JSON；这不是已更新安装包，也非原生 Tauri WebView 验收。

目前用户已关闭安装版。安装 exe 仍存在，但 Tauri/daemon 没运行、`127.0.0.1:11435` 未监听；本地 Laya PID 79984 仍监听 `127.0.0.1:18767`，临时开发服务 `5173` 已退出。按用户授权启动安装版的尝试被自动审批以 `blocked by policy` 拒绝；当前工具目录未提供 Tauri MCP。未尝试绕开审批。仍需在允许的启动路径恢复安装版后，用真实事件核实 390×844 窄屏行布局，并将本次源 UI 改动打进安装包后再验收。原生 Tauri WebView 与 DPI 适配仍不能由浏览器视口结果替代。

## 2026-09-26 01:36：新 UI 的 MSI/NSIS 构建与 MSI 载荷核对

`cargo tauri build --bundles nsis,msi` 以 exit 0 完成；Tauri CLI 为 2.9.6，build hook 重跑 `npm run build --prefix ui`，JS 产物为 `index-TdeG6B1Y.js`、CSS 为 `index-C4hssDtU.css`。生成 MSI SHA-256 `268F8820317FBECC7CA4D1631781F37D3CD6CF269F797D5F93E402870FB5E76A`，NSIS SHA-256 `BDBE3948ECB865E0FDDD527F20F0E9522B983BF67F709BF9C03A1790076B8409`。

将 MSI 管理映像只读提取到 `E:\tmp\jev-call-history-msi-extract-20260926` 后，逐一比对源码与提取文件：`index.html` SHA-256 `58BE849C432404419E8C228007C019E3B46C03A2E0CD982CA65B3BC9E04C820F`；CSS `D2A0771D53024E6D3A1FE8EA6AE3AA6DD58B7FD652EDD8561050CE11696F0FF3`；JS `46AFBAE2F030C319E7A0375E30E27F831B43134B5EA3F1A38F5B01F485331036`；shell `D22C709F9D2804132581757A426FA54691E614B8B78AAA1072909F1098B8E095`；daemon `E071CB195E7963E2F40F7CFF85C9EB148477CF13A0BD9FD78AB022CA1951264A`，全部与包内副本一致。新 MSI 没有安装；NSIS 是本轮同次构建产物，未单独提取或运行安装程序。

Tauri bundler 对 MSI/NSIS shell patch 都报告 `__TAURI_BUNDLE_TYPE variable not found`，并提示 updater 更新功能可能不可用。`src-tauri` 当前未发现 updater 插件依赖或注册，因此该项记录为构建警告，未据此宣称或测试 updater 功能。MSI/NSIS 成功产出；安装版 WebView、窄屏真实事件和托盘/数据库生命周期仍须运行验收，P5/P6 保持未完成。

## 2026-09-26 02:16：SQLite 持久 route trace 与跨重启活动游标

此前 request `detail` 在管理员 EventBus 中可见，但 EventBus 是内存环；调用者 API 虽读 SQLite，却没有 per-hop trace。现在 migration 007 在 `call_logs` 增加 nullable `request_id`、`http_status`、`route_trace_json`。新请求先在事务中写统计行并取得稳定的日志 ID，再将 `jev-<id>` 与 trace 保存到同一行；成功响应正文 metadata 和 `x-jev-request-id` header 返回相同 ID。记录的 trace 来自 `jev-core` 路由，不保存调用请求、问题/提示词、回答正文或 API key。

`GET /v1/admin/events` 与 `/v1/events/my` 均按 durable call-log 主键生成事件；零游标读取最近一页，增量轮询按 ID 向前。管理员实时 EventBus 事件也使用数据库生成的 ID，避免重启后游标错位。失败调用仍作为 `request` 类型展示，通过 `success=false` 和实际 HTTP status 形成可读失败摘要。迁移前的事件保留原字段并可从历史错误标记回读 HTTP status；旧行中的新字段为空。

测试经进程内 HTTP 实际验证响应体与 header request ID 相同，管理员事件与调用者事件含相同 request ID/trace；route trace 不含测试请求正文；删除对外入口并重建 daemon state 后，两种权限范围仍读到相同历史与 ID。另测试管理员 `since` 游标无重复、EventBus 乱序到达仍按持久 ID 排序，以及版本 5 数据升级经过 v6/v7 后原行/自增高水位保留。完整 Rust workspace 离线套件 **208 passed, 0 failed**；新增历史集成组 **4/4**、EventBus 定向测试 **3/3**。

随后重编 Windows release daemon 并替换 Tauri sidecar 输入，daemon SHA-256 `888A0FD6DDEDAC81146F2E162246C2BD89ECCD0C6B2D472C6996518349CC3C95`。新 MSI SHA-256 `4AD444CE6EF38ABF742BEEE77A38505F3B7DBB376082429787FF7C1A4182527C`，NSIS SHA-256 `FA8AD69400F059A13CF0CDBB5D7D648ABF555240C755F8B66E952FA1A1DD3861`。从新 MSI 管理映像提取的 index、CSS、JS、shell 与 daemon 五项 SHA-256 均和构建源匹配。新包尚未安装，故未验证正式数据目录迁移或原生 Tauri 行为；工具当前仍无 Tauri MCP，安装版也未运行。未执行被拒绝的桌面启动动作。

## 2026-09-26 02:55：失败路由 trace、持久历史与新安装包

此前成功 trace 已落库，但 core 错误映射前会丢弃 failover 的中间尝试。`Registry::invoke_with_strategy_traced` 现在给 success 和 failure 都保留 trace，同时 `invoke` / `invoke_with_strategy` 继续返回原来的 `JevError` 形状。失败路径按候选记录 provider/model、DAG hops、优先级、attempt、错误类别、上游 HTTP 状态、retryable 与决策；路由错误/能力跳过也有原因。没有复制原始错误正文、请求体、问题内容或凭据。

daemon 将失败 trace 与 HTTP status、上游调用数写入 `call_logs.route_trace_json`，并与 `jev-<log id>` / `x-jev-request-id` 关联。活动 API 继续返回可读失败摘要（provider/model/status/calls），路由逐跳细节保留在次要 `route_trace` 字段。core 用单测验证 429 候选转移后安全记录不可重试失败；HTTP/SQLite 集成测试验证失败 request ID、provider/model、状态、调用数与数据库 trace 一致，且私有上游诊断串不进入 trace。Rust workspace 离线回归 **210 passed, 0 failed**，`history_retention` **5/5**。

重新编译 release daemon 后，daemon 和 sidecar 输入 SHA-256 均为 `8A39D2E8A766C142C9046449CF7C7C51F17760EE954945BDB337C83023E362FE`。`npm --prefix ui run build` 通过，JS SHA-256 `46AFBAE2F030C319E7A0375E30E27F831B43134B5EA3F1A38F5B01F485331036`，CSS `D2A0771D53024E6D3A1FE8EA6AE3AA6DD58B7FD652EDD8561050CE11696F0FF3`。首次标准 Tauri 构建发现 beforeBuild hook 在 `ui` 子目录重复追加 `--prefix ui`；修正 `src-tauri/tauri.conf.json` 为 `npm run build` 后，UI hook 与未覆盖的 `cargo tauri build --bundles nsis,msi` 均成功。最终 MSI `11771F550F862051C67497630EA4964EA3C648A2C9D6165D06A4CF1448DE781E`，NSIS `7BF65D8FA30CEFA7F92296D7D668AA52EFB4D9E11ED0F30FC5A369F73C350283`。MSI 管理映像提取至 `E:\tmp\jev-failure-trace-msi-final-extract-20260926` 后，daemon、Tauri 主壳（SHA-256 `3A1F30E8CA07F2E63BFFADE3AE864637FB4514C2FB09144A7869A1FF55D45F7F`）、HTML、CSS、JS 全部逐文件 SHA-256 相同。

本轮未通过 Tauri MCP 重启安装版：当前可用工具集中没有 Tauri MCP；启动动作没有通过替代通道重试。新 MSI/NSIS 没有安装或运行，所以真实 Laya/Vercel 调用、正式数据目录 migration、WebView 屏幕尺寸与托盘/关闭生命周期仍需待安装版恢复后验证；本次 package/hash 与进程内测试不能替代原生联调。构建日志仍有 `__TAURI_BUNDLE_TYPE` updater 警告，仓库无 updater 插件配置，本次未验证 updater。

## 2026-09-26 03:20：当前工作树四页响应式复核

用户关闭了安装版 Tauri。当前没有 Tauri MCP，未尝试用替代通道启动桌面应用。为了继续检查布局，以当前 `ui/dist` 在 `127.0.0.1:4173` 临时预览：Dashboard 测 12 个视口（320×568 至 2560×1080，含 760×480、900×560、1000×650、1280×400）；Providers、Routing、Playground 各测 6 个视口（320×568、760×480、900×560、1024×768、1440×900、2560×1080）。30 组结果的 `documentElement.scrollWidth` 和 `body.scrollWidth` 都与视口宽度相等，没有页面级横向溢出。主体按高度纵向滚动。

Playground 在短窗内主要操作离表单首部过远：“运行全部”在 900×560 的 y=586–618，760×480 的 y=890–922，320×568 的 y=1129–1161；1024×768 与更大视口则处于首屏。这是用户要求改进窗口空间利用时应处理的实际摩擦，已补入 [Playground 多入口横向对比计划](../design/PLAYGROUND-COMPARISON-PLAN.md)：紧凑共享输入、题目区折叠/展开、操作区重排需一起设计，再用这些视口和真实多个目标复验。

该预览 daemon 不可达，公开入口与 provider 目标未加载，不能运行真实模型；所以这次只证明页面几何和滚动范围。没有生成正式截图。临时 4173 preview 和 viewport 覆盖随后关闭/重置；5173 被另一个项目 Team Sense 占用，未触碰。新的原生 Tauri WebView、首窗启动、托盘和关闭重启仍需后续验收，P5/P6 未完成。

同日按收紧后的 **900×560** 默认值，重跑 `native_initial_window_fits_each_attached_monitor`，实际三屏 100%/125%/150% 缩放下均满足窗口外框落在工作区内：DISPLAY1 150% 的内容区 900×560、外框 1372×896 位于 (274,-1084)；DISPLAY5 125% 的受限内容区 763.2×560、外框 972×747 位于 (-1026,-643)；DISPLAY6 100% 的内容区 900×560、外框 916×599 位于 (342,127)。隔离测试 **1/1 通过**。它仅创建隐藏原生 WebView 验几何，不启动安装版应用、daemon、托盘或完整页面；不能替代用户要求的安装版本地联调。

## 2026-09-26 03:51：演练场短窗可达性修正

前一轮 30 组静态视口检查后，直接调整了当前工作树 UI：紧凑/矮视口默认折叠共享输入，只显示 state 顶层字段和题目数；“展开输入”后恢复表单/JSON 编辑。窄屏/短窗使用“对外入口/直连上游”页签，一次只显示一个入口编辑表单，仍可重复切换并添加多个目标；桌面视口维持双类别并列。短窗将标题与示例选择合并，优先使用可视空间呈现目标与运行区。

最新生产 UI 的 Dashboard、Providers、Routing、Playground 四页共 30 组页面/视口检查均无 document/body 横向溢出。Playground 7 个视口结果如下：

| 视口 | “运行全部”位置 | main 可视区底部 | 结果 |
|---|---:|---:|---|
| 320×568 | y=439–471 | 539 | 首屏内 |
| 760×480 | y=371–403 | 451 | 首屏内 |
| 900×560 | y=395–427 | 531 | 首屏内 |
| 1280×400 | y=365–397 | 400 | 首屏内 |
| 1024×768 | y=605–637 | 733 | 首屏内 |
| 1440×900 | y=711–743 | 865 | 首屏内 |
| 2560×1080 | y=695–727 | 1045 | 首屏内 |

320×568 切换到直连上游后，按钮底 y=497、main 底 y=539；输入展开/收起及选择两类入口均实测往返。`npm run test --prefix ui` **21/21**，`npm run build --prefix ui`（含 `tsc --noEmit`）通过。`cargo tauri build --bundles nsis,msi` 成功：MSI SHA-256 `204281FF14C1BE8D875902647F85D55A8F2320CAD182A0B3498D84E16A289A51`，NSIS `B9CF2BA51775C1E65F0E4927A4E230F401BAA4373E8C5291DC2E4981DFBEFF68`。WiX `dark.exe` 提取到 `E:\tmp\jev-short-layout-msi-20260926`，shell、daemon、HTML、JS、CSS 全部与本次构建输入 SHA-256 匹配。MSI/NSIS 没有安装/启动；Tauri bundler 报 `__TAURI_BUNDLE_TYPE` 警告，dark.exe 有安装 UI ControlEvent 外键的 DARK1059 反编译警告。静态预览 daemon 不可达，不能调用真实模型；WebView/托盘/真实事件验收待后续，正式截图继续推迟。

### 2026-09-26 04:02：新包已核对，安装版仍关闭

收尾进程/端口检查：无 `jev-switch` 或 daemon 进程，4173、5173、11435 均未监听；5173 此前已识别为另一项目的 WSL 服务，本轮未触碰，收尾 WSL 检查也无该端口 listener。Laya 仍在 `127.0.0.1:18767` 监听（PID 79984）。新 MSI/NSIS 只构建与只读检查，没有安装或启动。当前工具集没有 Tauri MCP；此前启动安装版被自动审批以 `blocked by policy` 拦截，本轮未以其他通道重试，因此未完成安装版冷启动、真实 Laya/Vercel 新包请求、daemon SQLite migration 和托盘生命周期验收。


## 2026-09-26 04:28：安装版复起、Laya 历史摘要与升级待确认

按用户指示重新启动已安装的 `jev-switch`。原安装目录为 `E:\tmp\jev-nsis-install-20260925\app`；health 返回 200，build revision 为 `typesafe-confidence-cost-perms-20260925`，daemon 监听 `127.0.0.1:11435`。复用本机 Laya `127.0.0.1:18767` 完成一次 `laya-english` 真实调用：HTTP 200、206 ms、49 input / 0 output tokens、1 次上游调用，答案 `noul=0.3461`。实时响应含完整 route trace（selected provider/model `laya` / `laya-english`）；该旧安装版持久化活动记录的 detail 只含入口、provider、model、状态、耗时、调用数、usage 与未知成本，没有答案和 trace。

工作区前端已经对该事件做结构化摘要，并把原始 JSON 放在默认折叠的 disclosure 中。`npm test` 为 **21/21**，`npm run build` 通过。临时 Vite 页面连接同一个安装版 daemon；浏览器可访问性树显示“laya-english · 成功 HTTP 200 · 206 ms · laya → laya-english · 输入 49 / 输出 0 tokens · 1 次上游调用 · 成本未知”，其“展开原始 JSON”控件初始为 collapsed。临时 5173 服务在检查后已停止。

已安装包仍是旧 UI：安装目录 JS SHA-256 `D03B2AF6B75C178161E8016D563E514DFB68C0551152EE097AC570849F622BFD`，不含新摘要文案。新 MSI 内的 UI JS SHA-256 与当前 `ui/dist` 一致（`6C75A5A229333C8C3521219D053F0F7F33810CEB18A234B7A2895807617BCF57`）；应用配置和 SQLite 位于 `%APPDATA%\jev-switch`，与安装目录分离。升级包已核验并开始更新旧安装版，Windows UAC 仍待用户确认；原 Tauri/daemon 为安装而关闭，用户确认后需恢复启动并完成包内运行核验。当前可用工具没有 Tauri MCP 或原生桌面 UI 控制，因此本次 UI 可视检查使用的是临时浏览器源 UI，不宣称为 Tauri WebView 验收。没有打印或复制 API key。

## 2026-09-26 04:42：Vercel TypeSafe 官方契约与回归补充

按 `web-access` 流程检查 Vercel 官方 [TypeSafe API 文档](https://vercel.com/docs/ai-gateway/sdks-and-apis/typesafe) 与 [TypeSafe HTTP API 公告](https://vercel.com/changelog/ai-gateway-now-supports-typesafe-clients-and-http-api-for-jev)。文档当前确认 Gateway TypeSafe base URL `https://ai-gateway.vercel.sh/typesafe`、请求 `POST /typesafe/v1/systemone`、Bearer AI Gateway key，以及 Jev 原生请求形状；响应 `usage` 使用 snake_case，并以 `provider_metadata.gateway.gatewayCost` 报告 Gateway 成本。`upstream_vercel.rs` 的完整 base URL、原样 Jev body 和 Bearer auth 与文档一致。DTO 同时接受正式文档使用的 `provider_metadata` 与既有 camelCase `providerMetadata`，费用只读取明确的 `gatewayCost`，不以 market cost 或 token 数推算。

新增 `documented_snake_case_typesafe_response_preserves_usage_and_gateway_cost`，从 Vercel 文档风格 JSON 走完整 `VercelProtocol::incoming`，验证 model、noul answer、input/output usage、gateway cost 和 metadata 保留。`cargo test --manifest-path rs/Cargo.toml -p jev-adapters --locked --offline` 共 **29 passed, 0 failed**（27 unit + 2 integration）。`cargo fmt ... -p jev-adapters -- --check` 仍失败：格式差异遍及该 package 既有 `upstream_laya.rs`、`upstream_vercel.rs`、`vercel_protocol.rs` 与 retry wiremock；和早期工作树全局 rustfmt 未通过一致。本次未对整包做无关重排。尚未重跑真实 Vercel 请求；安装版升级仍在 Windows UAC 等待。
## 2026-09-26 05:17：Docker 镜像与同源 UI 冒烟验收

基于工作区同一源码构建 jev-switch:codex-acceptance-20260926，JEV_BUILD_REVISION=local-jev-switch-source-20260926。默认 Debian CDN 容器下载实测约 9 KB/s；改用 Debian 官方镜像名单收录的南京大学镜像后，运行层依赖安装约 11 秒完成。未改 Dockerfile，也未操作既有 Lingmou 容器。

隔离容器使用临时 tmpfs /data、独立名称和 127.0.0.1:11436。GET /health 返回 200、status=ok、version=0.1.0 和正确构建标识；GET / 返回 200，title 为 Jev-Switch — Multi-model decision router。从容器下载的 index-BY96iTcq.js SHA-256 与 ui/dist 文件完全一致：6C75A5A229333C8C3521219D053F0F7F33810CEB18A234B7A2895807617BCF57；验证包内有中文“展开原始 JSON”“成本：”“上游调用”文案。容器日志确认示例 Laya 装配，Vercel key 缺失按预期安全跳过；未向 Docker 配置真实密钥或请求上游。本次仅验镜像和同源静态 UI，没有实路由结论。

首个 detached 容器在健康检查后收到 SIGTERM 并正常退出；改成保持前台的唯一验收容器后，health、UI 与 JS 哈希全部通过，再显式停止清理。最终无 jev-switch-acceptance-* 容器，11436 无监听。既有 Lingmou 容器未变。

Tauri 更新仍等待 Windows UAC：consent.exe 存活，未见提权 msiexec、Jev 进程或 11435 监听。新版 MSI 安装、冷启动、正式 AppData 中调用历史与路由轨迹复核继续列为 P5 未通过项。

## 2026-09-26 05:28：发布门禁回归

当前工作树运行 cargo test --manifest-path rs/Cargo.toml --workspace --features ts-rs --locked，退出码 0；含 ts-rs 的 serde alias 解析 warning 未使测试失败。Tauri 壳 cargo test --manifest-path src-tauri/Cargo.toml --locked：10 passed、3 ignored；被忽略项分别需要兼容的实时 daemon 或 Windows 原生交互显示环境。UI node --test：21/21 通过；npm run lint --prefix ui 与 npm run build --prefix ui 均通过，生产资源为 index-Cp7fxIgV.css / index-BY96iTcq.js。

这些是当前工作树的组件与集成门禁，不证明最新版 MSI 已安装启动。真实安装版冷启动、托盘与配置恢复、安装后 AppData 的调用历史和路由轨迹仍等待当前 Windows UAC 完成；P5/P6 未完成。

## 2026-09-26 05:36：生成物空白修正与双配置 CI 门禁

全树 diff 检查发现 ts-rs 会在带字段级 Rust 文档注释的两个生成类型行尾写入空格。将 key 语义移入 ProviderInput/ProviderView 的类型级文档后，重新运行 ts-rs workspace 生成：前端类型继续带有关键说明，生成行不再含尾随空格，git diff --check 通过；npm lint/build 通过且 JS/CSS 文件名未变。

当前工作树重新通过 CI 的两条 Rust workspace 命令：默认 features 的 cargo test --workspace --locked，以及 ts-rs feature 的 cargo test --workspace --features ts-rs --locked。Tauri 壳测试为 10 passed、3 ignored，UI 测试 21/21，npm lint/build 均通过。ts-rs 仍发出 serde alias 解析 warning，但不影响生成门禁与测试结果。

本轮只完成可本地验证的 P5/P6 前置质量门禁，没有提交或推送；最新 MSI 的安装仍停在真实 Windows UAC 确认。正式桌面冷启动、真实模型路由、托盘生命周期及正式截图仍未完成。

## 2026-09-26 08:02：新版 MSI 正式安装版与模型路由联调

### 安装版与 AppData

用户已在 Windows UAC 安装确认并完成新版 MSI 安装。本轮启动安装目录 `E:\tmp\jev-nsis-install-20260925\app` 中的 `jev-switch.exe`：进程 PID 81208、标题 Jev-Switch；其子 daemon PID 83312。安装记录 ProductCode `{A01FC442-D138-4096-944C-7E80DD725222}`。`127.0.0.1:11435/health` 返回 200、version `0.1.0`、product `jev-switch`、API revision 1、`build_revision=null`。安装 shell SHA-256 `9BEA05596B3ACD42FDB1924F3EB44F65331F172CCC9F2ABB3104D12BDBBFB8D6`、daemon `8A39D2E8A766C142C9046449CF7C7C51F17760EE954945BDB337C83023E362FE`、UI JS `6C75A5A229333C8C3521219D053F0F7F33810CEB18A234B7A2895807617BCF57` 均和工作区 release 输入逐字节一致。以哈希和版本核对安装产物身份，不依赖 health 中空的 revision。

正式数据目录 `%APPDATA%\jev-switch` 仍有 `providers.toml` 与 SQLite。只读查询表明数据库 migration 1–7 均已应用；历史保留至 event 15。Vercel key 仅通过管理 API 的掩码状态检查；不打印、不复制明文。当前 11435 由安装版 daemon 持有；验收 Docker 的 11436、旧 demo 的 5173 均未监听；Laya `127.0.0.1:18767` 仍监听。

### 真实模型与历史追踪

通过安装版 `POST /v1/systemone` 向 `jev-vercel` 发出最小 Choice 请求，HTTP 200，response/request ID `jev-14`，选中模型 `typesafe-ai/jev`、provider `vercel`，1 次 upstream，337 input / 32 output tokens，1372 ms。实际 route trace 为 `jev-vercel → typesafe-ai/jev → vercel`，AppData event 14 保存相同 request ID、provider/model/status/usage/latency/trace；事件记录 `cost_usd=0.0`。调用返回 yes、confidence 0.99。

另经安装版公开入口 `laya-english` 调用本机 Laya 两次，分别 event 13 与 15，均 HTTP 200。event 13：190 ms，50/0 input/output tokens，request ID `jev-13`；event 15：209 ms，109/0 tokens，request ID `jev-15`。两条持久 route trace 均为 `laya-english → laya → laya-english`，包含一次成功候选与所选 provider/model。事件记录不包含请求正文、响应答案或密钥。

### 页面与横向比较

Providers 页显示两项启用的接入配置、添加/ TOML 导入/编辑操作及密钥掩码；Routing 的新建入口表单可选跟随默认、failover、race、load-balance、shadow。仅查看并关闭表单，没有改变正式配置。

通过当前安装 daemon 的 `127.0.0.1:11435` 静态 UI 浏览器视图检查：Routing 入口页可见 7 个服务入口卡片和新建/编辑等操作；“调用路由 DAG”读取当前配置的 9 条路由边，并显示公开入口、中间路由节点、provider 配置及 model ports。演练场选中同一输入下两个目标：公开入口 `laya-english` 与直连上游 `laya / laya-english`。点击“运行全部”后两列均完成；客户端耗时 222 ms / 284 ms，两列都返回 Laya 的 routing answer，公开入口列显示实际 trace。1280×720 页面截图呈现共享输入、两类入口配置和并列结果区，未再出现左下大片无内容空白。Dashboard 实际呈现 event 13/14 结构化调用摘要，原始 JSON disclosure 默认收拢。

这里的可视界面是由安装版 daemon 提供资源的浏览器视图，不是原生 Tauri WebView 截图。当前 Tauri 主窗口可见，96 DPI 下客户区为 900×560、外框为 916×599，位置 `(342,127)`；环境没有可调用 Tauri MCP，CUA 的 native app surface 未提供窗口控制。没有用替代 UI 自动化操作托盘。Playground 直连上游列有当前比较结果，但管理员持久活动日志仅记录公开入口请求；直连测试是否纳入统一历史仍需作为产品决定/后续工作。正常托盘退出、关窗后行为、完整退出再启动后的数据恢复、其他 DPI/多屏截图、P6 正式素材和提交推送仍未完成，故 P5/P6 不勾选完成。

用户明确要求后续验收串成连续流程，避免把登录或 UAC 作为中途等待步骤。本次 MSI 已完成安装；之后剩余只读和本地验证均无需再次提权，本轮未触发新的 UAC。

### 2026-09-26 08:46：直连上游活动持久化与请求关联

实现前的安装版证据显示，演练场直连 `laya / laya-english` 成功，但后台 `call_logs` 只有公开入口事件。当前源码为通过校验且实际发出的直连 provider 调用新增持久活动记录：`endpoint_id=direct:<provider config>:<model>`，带 provider/model/status/latency/usage/cost、`jev-*` request ID 与 `kind=direct_upstream` trace。调用正文、响应答案、密钥和上游原始错误 body 不落盘；失败历史只保留 HTTP 状态和目标元数据。

成功调用在响应 JSON 和 `x-jev-request-id` 响应头返回同一关联 ID；失败仍通过响应头返回 ID。CORS 对获准的 Vite 开发源暴露该响应头，跨源页面可读取。演练场结果卡显示成功 request ID 与直接目标路径，失败文案也显示响应头 ID。Dashboard 结构化活动摘要新增 request ID，原始 JSON 仍默认折叠。

临时内存 SQLite + 假 provider 集成覆盖直连成功写入、活动 API 可读、JSON/响应头/历史 ID 一致、直连失败可关联、HTTP 503 持久化和 prompt/error sentinel 不进入 trace。`cargo test -p jev-switch-daemon --test endpoints_runtime`：7/7；UI `npm test`：21/21；`npm run lint`、`npm run build` 与聚焦文件 `git diff --check` 全通过。生产 JS 输出 `index-rsyine2x.js`。本轮没有执行真实上游调用，也没有访问现有安装配置。

此为源码/构建验收，不是 08:02 安装版运行时的验证；当前 MSI 和运行中的 daemon 没有被替换或重启，故直连历史 UI 尚未在安装版上复验。将此项合入下一次统一打包与连续验收，未为它单独触发登录或 UAC。

## 2026-09-26 09:00：全量门禁及 MSI/NSIS 离线核验

当前工作树的默认 Rust workspace 测试和 `ts-rs` feature workspace 测试均通过；Tauri shell 测试 10 项通过、3 项因需原生 Windows 环境而忽略；UI 测试 21/21，lint 与 TypeScript/Vite production build 通过。`cargo tauri build --bundles nsis,msi` 成功。

发布制品 SHA-256：MSI `AF808B5F1C3151A600952CFE4D6759190F8F6B94DFCDD1911B78ADF43085F585`，NSIS `AD22835DFA59D157EABB08E7B19643E7A91868B778CEB593955D45FD642A6303`。使用 WiX `dark.exe` 将 MSI 解包到 `E:\tmp\jev-switch-goal-release-msi-20260926` 后，包内 Tauri shell、daemon、`index.html`、`index-rsyine2x.js` 和 `index-Cp7fxIgV.css` SHA-256 均与当前本地构建输入一致；解包退出码 0。`DARK1059` 报告的是安装 UI 表关联缺失，不影响载荷提取。构建仍提示未设置 `__TAURI_BUNDLE_TYPE`；仓库没有 updater 插件，因此没有宣称 updater 已验证。

只读检查显示安装版 daemon PID 83312 仍从 `E:\tmp\jev-nsis-install-20260925\app\jev-switch-daemon.exe` 服务，health HTTP 200，旧 UI JS 为 `index-BY96iTcq.js`。本轮未安装新包、未重启旧实例、未触发 UAC 或认证等待；新源码中的直连上游历史记录尚未在该安装版复验。Tauri WebView、托盘和原生 DPI/多屏仍待最终连续验收。

## 2026-09-26 09:03：隔离原生窗口多显示器几何

对当前 `src-tauri/src/window_layout.rs` 显式运行隔离隐藏 WebView 测试 `native_initial_window_fits_each_attached_monitor`：1/1 通过。测试创建真实 Tauri 配置定义的隐藏 WebView 窗口，未启动 daemon、托盘、正式安装实例或访问用户数据；在三台连接的显示器上逐屏验证工作区边界、居中和最大逻辑客户区，显示缩放分别为 1.0、1.25、1.5，覆盖副屏负坐标。记录见 `E:\tmp\jev-window-multimonitor-20260925\native-multimonitor-test.log`。

该结果证明当前生产窗口尺寸/定位函数能在这些显示器几何下工作；它不验证正式安装版启动、多显示器间人工拖动、四页内容的 WebView 视觉重排或 DPI 截图。此前 14 项隔离 HTTP 检查使用的是另一版 daemon（SHA-256 `F8534846040B598E8D791FB5A46E19D3A00166AD5950D4651575B260F020F779`），全部针对 synthetic local HTTP fixture，未接真实上游；作为历史 HTTP 冒烟记录，不计为本次 MSI 载荷验收。

## 2026-09-26 09:08：当前 daemon 与 UI 的隔离端到端验证

直接启动本次 release daemon（SHA-256 `8A39D2E8A766C142C9046449CF7C7C51F17760EE954945BDB337C83023E362FE`），配置独立 `127.0.0.1:11439`、独立临时 providers 配置和 SQLite；上游使用进程内本机 synthetic HTTP fixture，不用任何真实 key 或账号。启动时移除继承环境内匹配 `API_KEY`、`TOKEN`、`SECRET`、`PASSWORD` 及厂商凭据的变量。静态首页、`index-rsyine2x.js`、`index-Cp7fxIgV.css` 的 SHA-256 分别与 MSI 解包内容完全一致。

**14/14 场景通过**：验证当前二进制身份与 health；用管理 API 创建调用 token 和公开入口；三次请求都到达本机假上游并返回可追踪路由；强制终止/重启后路由和 token 可用、活动历史 3 条且 event IDs 稳定；禁用入口时公开调用为 404，启用后恢复；拒绝环路配置且保留原图；删除入口后请求为 404，历史和累计统计在再次重启后仍保留。结果 JSON 位于 `E:\tmp\jev-current-daemon-acceptance-20260926\result.json`，SQLite、providers 文件、daemon log 也都在该目录。结束复查：daemon 进程 0 个，11439 无监听。

这补上此前隔离回归版本 SHA 与当前 build 不一致的问题，并证明当前 daemon、当前 UI 静态资产、持久配置和事件库在独立数据目录下共同工作。它不更改 `%APPDATA%\jev-switch`，不访问已安装实例、不安装 MSI、不验证原生 Tauri shell、不请求真实 Laya/Vercel，也不覆盖托盘/正式窗口验证。

## 2026-09-26 09:16：原生 Tauri 壳复用当前 daemon

显式运行原本默认忽略的 `sidecar::tests::isolated_tauri_shell_reuses_live_daemon_and_leaves_it_running`，1/1 通过。测试先探测现有 `127.0.0.1:11435/health`，再创建独立 WebView2 profile 和隐藏 Tauri 测试壳；本地 UI 成功加载。注入的 sidecar 启动函数若被调用会直接失败，实际没有触发；退出时 `ShellState.child` 仍为空。

结束后 health 仍为 HTTP 200，daemon PID 83312 与其原安装路径不变；测试 profile 在 `E:\tmp\jev-tauri-reuse-current-20260926`。该项证明当前壳代码可以识别兼容内核并复用其 HTTP 服务、完成 WebView 导航，同时不会误停已有 daemon。被复用 daemon 的 JS 仍是旧版，故该测试不验证本次 MSI 页面、冷启动 sidecar、托盘退出或正式 AppData 配置恢复。

## 2026-09-26 09:26：原生 Tauri 壳 spawn 与退出收尾

新增并显式运行忽略测试 `sidecar::tests::isolated_tauri_shell_spawns_and_stops_its_sidecar`。测试用当前 Tauri 壳测试 harness 建立隐藏 WebView，创建独立且 disabled 的 Laya 示例配置，强制启动分支为 `RuntimeProbe::Unavailable`，并通过工作目录中的当前 sidecar 与 `ui/dist` 启动临时服务 `127.0.0.1:11437`。`GET /health` 通过版本/API 身份验证，`GET /` 为 HTTP 200 且包含当前构建资源 `index-rsyine2x.js`。整个过程不使用真实 API key，不发送模型请求。

在 `RunEvent::Exit` 中执行生产 `shutdown(&mut child)` 后，测试断言 ShellState 已收回子进程、11437 变为不可达。测试 1/1 通过；安装版 daemon PID 83312 与 11435 HTTP 200 未改变。临时配置、数据库、WebView2 profile 与 daemon 日志在 `E:\tmp\jev-tauri-spawn-current-20260926-run2`。常规 `cargo test --manifest-path src-tauri/Cargo.toml --locked` 为 10 passed、4 ignored，`cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` 通过。

这证明当前 Tauri 启动器能在隔离配置里拉起对应版本的 sidecar、从它提供的静态目录读取当前 UI，并在壳退出时收尾自己拥有的 child。它不触发安装，不操作托盘菜单，也不覆盖真实 AppData、用户配置升级与恢复、四页 UI 视觉或真实 Vercel/Laya 模型调用。

## 2026-09-26 09:32：当前原生首窗单屏几何

显式运行 `window_layout::tests::native_initial_window_stays_inside_current_work_area`：1/1 通过。真实隐藏 Tauri/WebView 窗口在当前主屏 scale 1.0、工作区 1600×852 下，客户区为 900×560，外框 916×599，位置 `(342,127)`，没有超出工作区。隔离 WebView2 profile 位于 `E:\tmp\jev-window-single-current-20260926`；此测试不启动 daemon/托盘、不访问用户数据，也不为窗口截图。

截至此检查点，四个默认忽略的原生集成项均已各自显式实跑：已有 daemon 复用、当前 sidecar 启动/退出、单屏首窗几何、多屏几何。它们分别有独立证据；仍没有正式安装包四页 WebView视觉、托盘菜单操作、保存/重启窗口几何的证据。

## 2026-09-26 11:13：旧安装载荷诊断与同源便携构建

正式 AppData `%APPDATA%\jev-switch\jev-switch.db` 的 `call_logs` 共有 17 行，最新公开入口记录为 `jev-16`（Laya `laya-english`，HTTP 200，46/0 tokens，196 ms）和 `jev-17`（Vercel `typesafe-ai/jev`，HTTP 200，304/21 tokens，1341 ms）；两条均含持久 route trace。早前对旧安装 daemon 的一次直连 Laya 请求返回 HTTP 200，但响应没有 `x-jev-request-id` / JSON `request_id` / `route_trace`，DB 没有新增行。旧 daemon SHA-256 `8A39D2E8A766C142C9046449CF7C7C51F17760EE954945BDB337C83023E362FE` 中没有直连持久追踪代码标记。由此定位到制品时间差：此前核对的是包与当时 sidecar 输入哈希一致，并非与后来新增的直连追踪源码一致。

当前源码 release daemon 重建成功，SHA-256 `E6E84FCBB796294261409927B3E5E0B0E260A6B5522E84BD76E53DDC7C8279DD`；`endpoints_runtime` 集成测试 7/7，UI 21/21，UI lint 和 Tauri 生产 build hook 均通过。修复 `src-tauri/tauri.conf.json` 的 `beforeBuildCommand` 为 `npm run build --prefix ui` 后，MSI build 成功。MSI SHA-256 `E47DACAEB5DE023A98295A9F5A47E48CFA70DB5B2605E215F083ED5673FED75E`，ProductCode `{11966F72-9DED-420E-9BCE-3AD090800994}`，UpgradeCode `{989320D3-86BE-51E7-84B6-DD8BDB4F0C42}`。WiX `main.wxs` 将 shell 指向本次 `src-tauri/target/release/jev-switch.exe`，daemon 指向 `E:\tmp\jev-switch-daemon.exe`，后者与 release Rust daemon / Tauri sidecar hash 同为 `E6E84FCB…`；UI 文件源指向本次 `ui/dist` 的 index/JS/CSS。当前 MSI 未安装。

新加的 `scripts/package-portable.ps1 -Destination <new directory>` 在本机成功运行，生成 `E:\tmp\jev-switch-portable-20260926-verified`。portable `jev-switch.exe` SHA-256 `6078248DC7FAB50D3EA755B413A3D6DCF85CC419C0F34E0D90B6FF2C77B92B63`；`build-manifest.json` 比对 sidecar/daemon 与静态资源 hash；目录包含壳、sidecar、`resources/ui/dist`、sibling `ui/dist` 和运行说明。便携目录与 MSI 清单引用同一组 build outputs；它是文件夹式运行时，不是自包含单文件 exe。

为给便携实例让出 `127.0.0.1:11435`，停止了本轮此前启动的旧安装 Tauri PID 51424 与 daemon PID 46508；AppData 配置/DB 保留、历史仍 17 行，Laya `127.0.0.1:18767` PID 79984 仍运行，`5173`/`11435` 无监听。启动便携 `jev-switch.exe` 的系统自动审批返回 `blocked by policy`，故本轮没有启动新程序或发出新的 Laya 请求，也没有换用其他执行方式。Tauri MCP 不在本会话工具中，CUA 原生应用清单为空。该记录当时提及的便携运行说明和 `jev-switch.exe` 路径为 `E:/tmp/jev-switch-portable-20260926-verified/RUN-PORTABLE.txt` 与 `E:/tmp/jev-switch-portable-20260926-verified/jev-switch.exe`；它们是本机临时产物路径，不是仓库文件链接。启动后待验 health、加载资源、真实直连 request ID 与 Dashboard/SQLite 关联、原生窗口和托盘。P5/P6 未完成。

## 2026-09-26 12:33：用户启动便携版、Laya/Vercel 路由与 Docker cloud 回归

用户手动启动 `E:/tmp/jev-switch-wix-verified-20260926-114045-417/jev-switch.exe` 后，Tauri shell PID 38364 与其 daemon PID 85628 均从该便携目录运行；11435 health 返回 `status=ok`、`product=jev-switch`、`api_revision=1`。Windows 进程信息显示 `Jev-Switch` 窗口存在且响应。该实例使用 11:40 shell（SHA-256 `B6D0538D…0CF3AB9`），其 daemon 与 UI 资源 hash 和自身 manifest 一致，并与 11:50 候选中的 daemon/UI 相同；11:50 shell SHA-256 为 `7B75629E…6E088BE`，尚未在本机运行，故本段不把 11:40 壳验证写成最新壳验证。

### 本地路由与 Vercel 上游

本机 Laya 测试服务 `127.0.0.1:18767/health` 返回 ok，CUDA english 与 multilingual checkpoint 已加载。公开模型入口 `laya-english` 调用成功：HTTP 200、`request_id=jev-18`、provider `laya`、upstream model `laya-english`、一个上游调用、190 ms、66 input / 0 output tokens；route trace 与 request ID 同步写入 `%APPDATA%/jev-switch/jev-switch.db` 的第 18 条 `call_logs`。

直连 Playground 路径也实际调用同一个 Laya 模型：`POST /v1/admin/providers/laya/invoke` 返回 HTTP 200、`request_id=jev-21`，`route_trace.kind=direct_upstream`、provider `laya`、model `laya-english`、单次上游调用、179 ms。数据库第 21 行以 `direct:laya:laya-english` 记录；刷新 Dashboard 后，最近调用摘要可见 request ID、HTTP 状态、耗时、tokens 和折叠的原始 JSON。此项确认直连候选同样关联到运行历史。

进一步在演练场点击“运行全部”，当前 UI 同时运行公开入口 `laya-english` 和直连 `laya / laya-english`。两列均成功，展示结构化答案与 usage；客户端耗时分别为 209 ms 和 308 ms。数据库新增公开调用 `jev-22`（SQLite 第 22 行，`strategy=failover`）和直连调用 `jev-23`（第 23 行，`kind=direct_upstream`）；重载 Dashboard 后，两条摘要均可见 request ID、目标、耗时、tokens，原始 JSON 保持折叠。

通过公开入口 `jev-vercel` 调用同一 Vercel 配置成功：HTTP 200、`request_id=jev-19`、实际路径 `typesafe-ai/jev → vercel`、一个上游调用、928 ms、324 input / 23 output tokens；trace 落入第 19 条。密钥只在 daemon 使用，GET/UI 仍仅显示掩码。SQLite 记录的 `cost_usd=0.0` 来自 adapter 读取到的显式 gateway cost；本次未独立对账供应商账单，因此不把该值解释为账单事实。

检查发现当前两个 Provider 的 `models` 列表均为空，尽管路由已使用对应上游模型。经 `PUT /v1/admin/providers` 将已验证的 Laya 模型 `laya-english`、`laya-multilingual`、`laya-router` 与 Vercel 模型 `typesafe-ai/jev` 写入模型清单；PUT 省略 key 字段，保持 Vercel key 和原路由。回读确认 Laya `api_key_set=false`、Vercel `true`、9 条路由完全未变，配置权威为 SQLite 且 TOML 无漂移；随后追加 Laya smoke request `jev-20`，HTTP 200、单次上游调用。浏览器 Providers 页回读为 3 个 Laya 模型 / 1 个 Vercel 模型。

### 界面核对与范围边界

Codex in-app browser 读取当前 daemon 提供的页面：Routing 默认选中“入口”，可切到“调用路由 DAG”；图含 9 条边、公开入口与提供商模型端口。Playground 实际运行公开 `laya-english` 与直连 `laya / laya-english` 两个目标，结果先显示结构化答案、实际路径、耗时和 usage，“原始响应”折叠。Dashboard 的 `jev-18` / `jev-19` / `jev-21` / `jev-22` / `jev-23` 行以摘要显示 request ID、路由和 tokens，原始 JSON 通过“展开原始 JSON”打开。Windows 存在原生 Tauri 窗口，但 CUA native app inventory 为空且没有 Tauri MCP；上述 DOM/AX 证据来自浏览器页，不作为原生 WebView 截图或交互验收。

WSL Ubuntu 中确认 Docker 28.4.0、Compose 2.39.2 可用，修正了 11:51 只查 Windows PATH 后的“当前无 Docker”判断。`docker compose config --quiet` 通过，当前工作树构建镜像 `jev-switch:codex-p5-20260926-1206`。隔离 cloud 容器以 11436 回环端口和临时卷运行：health 与同源首页 200；未登录 admin 与无调用 token 的 `/v1/models` 均为 401；admin login 后可读两个 Provider、caller token 可读 5 个公开模型。创建测试入口、重启容器后入口持久保留，caller token 继续有效，重启前的 admin session 失效且需重新登录。测试专用容器、卷与端口均已清理；没有触碰其他容器。

P5 仍待：最新 11:50 shell 的运行验收、直接观察正式 Tauri WebView 的四页/主题、托盘菜单退出与配置恢复、多 DPI/屏幕系统级走查。P6 正式截图与提交/发布收尾仍按“最后一次性完成”执行。
