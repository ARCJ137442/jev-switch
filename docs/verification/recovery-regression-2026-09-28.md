# Windows 重启后本地运行恢复核对

日期：2026-09-28

## 运行状态

Windows 重启后没有重复启动健康服务，以避免再次增加内存压力。实际监听与身份如下：

| 地址 | 服务 | 核对结果 |
|---|---|---|
| `127.0.0.1:8808` | Kev-0.8B 官方 `kev.serve` | `/v1/models` 返回 `kev-latest`，CUDA、FP32、temperature `2.3510958125672174` |
| `127.0.0.1:18765` | Laya multilingual multi-server | `/health` 返回 `ok`，`loaded` 包含 `english`、`multilingual` |
| `127.0.0.1:11439` | Jev Switch 源码 daemon | `/health` 返回 `ok`，`/v1/models` 广告 `kev08-local`、`laya-local` |

LM Studio 进程已恢复但没有自动加载模型。核对时 Windows 可用物理内存约 `7.3 GB`；Kev 与 Laya 已占用显存/内存，因此本轮没有再加载 4B 模型。

## 回归验证

- Jev Switch Rust workspace：使用隔离的 `.tmp/recovery-target`，避免运行中的 `jev-switch.exe` 锁定默认 target；全部测试通过。
  - `jev-protocol` 25；`jev-adapters` 31；`jev-core` 57；daemon 单元 74；daemon 集成/HTTP 等测试也全部通过。
- Decision Lab Python 辅助测试：`8 passed`。
- 恢复后 10 条生活记录经 `kev08-local` gateway：10/10 格式有效、0 路由错误，准确率 `8/10`；这是 smoke，不替换 canonical 80 条结果。
- Rime 2 个样本、A/B/C/D payload，经同一 gateway：
  - Kev：`char` Top-1 `62.5%`，`word_code` Top-1 `75%`，8/8 请求有效；
  - Laya：`char` Top-1 `37.5%`，`word_code` Top-1 `0%`，8/8 请求有效。
  该结果仅用于确认恢复后的协议与路由，样本太小，不能当作完整模型排名。

## 未完成与边界

- WSL 已安装 FastJev `0.2.0` editable 包，`fastjev-serve --help` 可用；Linux CUDA `llama_cpp_python` wheel 下载在恢复时多次超时，因此 GGUF 后端尚未安装，也未启动 FastJev 服务。继续安装时应使用 IDM 下载 wheel 后离线安装。
- Kev-4B 尚无本地 Transformers 权重。所需 Qwen3.5-4B-Base 两个 shard 合计 `9,319,828,056` bytes，且 C 盘仅剩约 `30 MB`；下载必须将 Hugging Face cache 指向 E/G 盘。
- 本机下载目录中的 `Qwen_Qwen3.5-4B-Q4_K_M_3.gguf` 是普通 Qwen3.5-4B Q4_K_M，不能用于加载 Kev-4B 的 PEFT adapter；另一个未确认的临时下载文件是 JevK5-9B GGUF 的硬链接，也不是 Kev-4B。具体文件名和路径属于本机私有环境，不纳入公开记录。

证据输出：

- `jev-decision-lab/life-series-benchmark/results/recovery-smoke-kev-switch-2026-09-28.json`
- `jev-decision-lab/life-series-benchmark/results/recovery-rime-kev-laya-2026-09-28.json`
