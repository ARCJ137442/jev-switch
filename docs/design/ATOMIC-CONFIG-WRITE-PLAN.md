# 原子配置写入计划

**状态：**路由原子 transaction Phase 1 已实现；入口创建/更新已迁移为边级操作；显式备份导入仍保留为恢复边界。
**触发原因：**普通入口写入曾内嵌 `routes` 整表字段，容易覆盖同一入口之外的变更。

## 目标

所有面向 UI、CLI 和 Agent 的普通配置修改都必须是领域资源级的原子操作，或显式的批量恢复/Graph transaction。客户端不得依赖“读取整表 → 本地修改 → 整表覆盖”。

## 必须覆盖的资源

- provider：已有单资源 `PUT/DELETE /providers/{id}`；继续保持省略字段保留既有值、保存失败不改变运行时。
- provider lifecycle：已有单资源 `PUT/POST`；命令正文不回显，状态和审计独立保存。
- public endpoint：已有 endpoint create/update/delete；入口路由变更使用 `route_operations`，必须与入口元数据同一事务。
- route edge：通过原子 transaction 提交单边创建、单边更新、单边删除；每次操作只影响一个稳定 `edge_id`。
- graph：只有在确实出现跨多个节点/边的用户动作时，才评估更大范围的 Graph transaction。

## 事务不变量

一次成功写入必须同时更新 SQLite 快照、内存 Router、入口索引和 Graph projection；任意校验、环检测或持久化失败都必须全部回滚。

本轮不引入额外版本号或 ETag；管理端写入由 SQLite 写锁串行提交。删除前提供影响预览；历史调用记录不随配置删除级联丢失。

## 非法状态矩阵

普通写入先在内存副本或 SQLite 事务中完成完整校验，再产生唯一一次持久化提交。提交前失败不得留下部分元数据、边或适配器；提交后只允许出现“持久快照已合法、运行时正在同步”的短暂阶段，不能把未校验图安装到 Router。

| 写入 | 可能非法状态 | 拒绝原因 | 处理方式 | 验证证据 |
|---|---|---|---|---|
| route transaction | 空节点、重复边身份、未知 provider/别名、环、批量操作中途找不到目标边 | 目标图无法被 Router 安全执行，或操作序列不是当前快照上的合法变更 | 副本逐项应用，随后做字段/环/引用校验；任一失败返回 400/404/409，旧 SQLite、Router 和 Graph projection 保持不变 | `put_routes_cycle_is_400_with_cycle_message`、`put_routes_rejects_unknown_right_node`、失败快照断言 |
| provider PUT | 空 ID/kind/base、未知 adapter、非法 lifecycle、更新后已有边失去终点 | provider 配置或现有 DAG 不可解析 | 在 SQLite 写锁内读取最新快照，合并单 provider，清理只因该 provider 失效的分支，完整校验后保存；失败不替换适配器 | provider 字段校验、删除级联与路由回读测试 |
| provider DELETE | 删除 provider 后直接边、别名链或入口分支悬空 | Router 可能保留不可达终点或公开入口伪成功 | 同一快照中收缩失效分支；保留仍能到达其他 provider 的分支和调用历史 | `put_providers_deleting_provider_cascades_routes_and_refreshes_router` |
| endpoint create/update | `route_operations` 修改了其他入口、重命名后边 ID 未映射、入口策略非法、元数据已写而边校验失败 | 入口元数据和路由图会分裂 | metadata 与边操作共用 SQLite transaction；重命名先同步引用再应用操作，完整图校验失败即 rollback | `endpoint_writes_reject_invalid_graphs_without_changing_runtime_or_storage`、restart recovery |
| endpoint DELETE | 删除入口后中间别名悬空，或误删另一条有效分支 | 公开模型列表和已有入口可能被错误影响 | 事务内仅级联删除已断开的分支；保留独立 provider 分支和历史调用记录 | endpoint deletion/history retention tests |
| lifecycle/host settings | lifecycle 配置能保存但 provider 状态不合法，或主机命令开关变更覆盖路由 | 服务控制状态与路由配置互相污染 | lifecycle 与 host gate 只更新其领域字段；校验失败不写快照，路由快照不变；命令正文不回显 | lifecycle and host-command round-trip tests |
| JSON/TOML import | 导入含环、未知引用、缺失 adapter、非法 provider，或导入过程中任一步失败 | 批量恢复可能覆盖当前可用配置 | 仅显式 `confirm=true` 进入；先解析、构造 adapter、校验完整图，再保存快照；失败保留旧配置 | rejected import snapshot/restart tests |

### 提交后同步失败

SQLite 快照提交后，Router、adapter registry 和入口索引必须按同一快照刷新。若刷新阶段发生 I/O/锁错误，不能声称“已回滚”：接口返回带上下文的 500，持久快照仍是唯一合法真值，下一次加载会重新同步。后续新增写入必须复用这一边界，禁止另写一个只更新 TOML、只更新内存或先替换 Router 再落盘的路径。

## 兼容迁移

保留 GET `/v1/admin/routes` 作为读取兼容接口。整表 `PUT /v1/admin/routes` 已移除并返回 405；带确认的 JSON/TOML import 仍是明确的管理员恢复流程。当前替换为：

```text
POST   /v1/admin/routes/transaction     # 原子提交一组边操作
```

当前已提供 `POST /v1/admin/routes/transaction`，并由入口 `route_operations` 复用：在运行时快照副本上应用 `create/update/delete`，通过完整图校验后一次写 SQLite 并热替换 Router；失败保持旧快照。provider 凭据仍只能通过 provider 资源 API 修改，不能混进图事务。

## 验收门槛

- UI 增加/修改/删除一条边不会发送完整 routes 数组。
- 环、未知 provider、未知 endpoint、重复 edge ID 均不改变 SQLite 或内存 Router。
- 重启后配置版本、路由、公开模型列表和 Graph projection 一致。
- Agent/CLI 能通过 capability discovery 找到资源、scope、风险、确认和审计字段。
