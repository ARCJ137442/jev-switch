# JevK5-4B LM Studio / Jev Switch 联调核对

日期：2026-09-28

## 目的与边界

本轮验证本地已有 JevK5-4B Q4_K_M 权重是否能经 LM Studio 进入 Jev Switch。权重位置使用本机私有配置，不在公开记录中展开。LM Studio 当前版本没有旧版 llama.cpp `/tokenize`、`/completion` 原生 logits 接口，因此新增了 bridge 的 `--backend lmstudio` 模式：通过 OpenAI Chat API 加 assistant prefill 取得单个选项字母。

这条兼容路径只提供 greedy Top-1 和 one-hot 概率，`confidence=1.0` 明确标记为未校准；它不是 JevK5 原生 option-logit 结果，不能与 9B raw-logit benchmark 的概率、ECE 或风险覆盖率混算。

## 资源与直接 smoke

- LM Studio 索引键：`jevk5@q4_k_m`；加载实例：`jevk5-4b-smoke`。
- 4096 context、单并发、全 GPU offload；估算 `3.03 GiB`，实际加载约 `2.52 GiB`。
- 生活记录 10 条直接 bridge：10/10 有效，标签一致率 `70%`，平均 `435.4 ms`，P50 `408.5 ms`。
- Rime 2 个样本、A/B/C/D payload：8 条 `char` Top-1 `25%`，8 条 `word_code` Top-1 `50%`，平均 `606.7 ms`，0 错误。
- 完整 80 条生活记录经 Jev Switch：`44/80 = 55.0%`，0 格式错误，平均 `439.7 ms`，P50 `406.1 ms`。

与同一批 Laya gateway 记录逐条配对：JevK5-4B 单独胜出 `31` 条，Laya 单独胜出 `5` 条，双方同对或同错 `44` 条。JevK5-4B 的代表性胜出包括：`87361fb3ba`（时间与行动，对 Laya 的安放与整理）、`f952ff2426`（Agent，对 Laya 的刨根问底）、`d1afb39acf`（写作与记录，对 Laya 的见自己）。Laya 单独胜出的 5 条主要集中在安放与整理/需求整理边界，例如 `92c13eeac8`（需求整理）。这仍是人工标签一致性，不是客观任务成功率。

上述都是小样本 smoke；80 条人工标签仍只有一致性含义，不能当作生产 outcome 或模型最终排名。

## Jev Switch 实际联调

临时注册了：

- provider `jevk5-4b-lmstudio`，`kind=typesafe`，base `http://127.0.0.1:8783/v1/systemone`；
- endpoint/model `jevk5-4b-local`，route `jevk5-4b-local -> jevk5-4b-lmstudio`，上游模型 `jevk5-4b-lmstudio`。

`GET /v1/models` 正确广告该模型；经 `POST /v1/systemone` 的 gateway smoke 返回 HTTP 200，route trace 记录：

```text
selected_provider = jevk5-4b-lmstudio
selected_model    = jevk5-4b-lmstudio
upstream_calls    = 1
```

3 条生活记录经 gateway 全部格式有效，平均 `304.7 ms`；完整 80 条结果见下方证据文件。验证完成后已删除临时 endpoint/provider，并卸载 `jevk5-4b-full`；当前 LM Studio 无模型加载，避免残留显存占用。

证据文件：

- `jev-decision-lab/life-series-benchmark/results/local-jevk5-4b-lmstudio-smoke-10.json`
- `jev-decision-lab/life-series-benchmark/results/rime-jevk5-4b-lmstudio-smoke-2.json`
- `jev-decision-lab/life-series-benchmark/results/local-jevk5-4b-lmstudio-switch-smoke-3.json`
- `jev-decision-lab/life-series-benchmark/results/local-jevk5-4b-lmstudio-switch-80-onehot.json`
