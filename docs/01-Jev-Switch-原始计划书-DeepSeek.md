# 「Jev-Switch」项目计划书

**交接对象**：GPT 5.6（实现 Agent）
**撰写目的**：完整交接需求、调研结论与战略判断，供后续实现决策使用
**前置说明**：本计划书中的“需求全文转录”部分为项目发起人的原始表述，未做任何修改。其余部分为对话过程中形成的调研结论与方案设计，需与原始需求对照阅读。


## 一、需求全文转录

以下为项目发起人对「Jev-Switch」的完整原始需求表述，逐字转录：

> 我想要的主要是能
> 1. 对外提供一个与TypeSafe Jev兼容的HTTP服务器
> 2. 保存加载与切换Jev的上游提供商，比如Vercel、TypeSafe官方、OpenRouter等
> 3. 在此之上，有整合其他类型的「类Jev服务」的能力，比如 https://huggingface.co/convaiinnovations/laya 这种
> 4. 提供某种形式上的「LLM-Jev」转接服务，乃至可以微调转接方式
> 这样的「Jev-Switch」，不与LLM Agent有关

**需求边界澄清**（来自后续讨论）：
- 该项目的HTTP服务端定位是“类似LM Studio那样的HTTP代理”，即本地运行、对外暴露统一接口、对内可切换后端。
- “不与LLM Agent有关”指：它不是Agent工具链的一部分（不参与工具选择、不拦截Agent请求），而是一个独立的协议网关。
- 后续补充判断：“需求应该是存在的，并且要注意的应该是‘被现有的LLM提供商吞并’，也就是CC Switch直接提供了‘Jev API’的状况……我们的HTTP服务端主要是类似LM Studio那样的‘HTTP代理’，后续如果CC Switch支持了Jev格式，可以依然存活。”


## 二、调研快照：现有项目对四条需求的覆盖情况

### 2.1 已确认的关键项目

| 项目 | 类型 | 对需求1 | 对需求2 | 对需求3 | 对需求4 |
|---|---|---|---|---|---|
| **sys1**（alvarobartt/sys1） | Rust，System One兼容API服务器 | ✅ 暴露`/v1/systemone`和`/v1/decide`，专为Laya设计 | ❌ 单后端（Laya），无多上游 | ⚠️ 仅支持Laya，未抽象为通用“类Jev”后端 | ❌ 无LLM→Jev能力 |
| **jev-accounts-hub**（antTing） | Go，多账户管理器和API网关 | 未知（无公开文档） | ⚠️ 多账户管理，可能涉及多Key切换，但是否多提供商未知 | ❌ 未提及 | ❌ 未提及 |
| **LLM2Jev**（Yinsongxu） | Python，本地LLM适配为Jev决策引擎 | ✅ 通过`llm2jev-serve`暴露`POST /v1/systemone` | ❌ 单后端（本地LLM） | ⚠️ 适配的是Hugging Face因果语言模型，非通用“类Jev服务”抽象 | ✅ prefill-only binary inference实现LLM→Jev转接 |
| **jev-life**（ARCJ137442） | TypeScript，游戏+broker | ⚠️ 内部有Jev兼容契约，未对外暴露为独立服务 | ✅ `DecisionBackend`抽象支持多后端，但切换为配置级而非请求级 | ⚠️ 架构预留（`DecisionBackend.kind`含`llm-json`/`llm-tool`） | ✅ broker是完整的LLM→Jev转接实现 |

### 2.2 竞争格局的关键发现

**cc-switch不会直接吞并Jev格式网关。** cc-switch的核心是配置管理工具，其代理功能做的是API格式转换（Anthropic↔OpenAI），而非Jev格式的`state + questions → answers`抽象。Jev格式与Chat Completions在结构上完全不兼容，cc-switch要让Agent用上Jev，需要做的是协议转换——这是网关的职责，不是配置管理工具的职责。cc-switch与Jev-Switch是互补关系，不是替代关系。

**真正的吞并风险来自LiteLLM和Vercel AI Gateway。** LiteLLM已经在v1.103.0-rc中加入了TypeSafe pass-through，客户端将`https://api.typesafe.ai`替换为`LITELLM_PROXY_BASE_URL/typesafe`即可，响应体原样返回。Vercel AI Gateway上线24小时内近13%的付费团队使用Jev。但这两个网关的入口格式是OpenAI/Anthropic兼容格式，不是Jev原生格式。

**Jev原生格式网关这一生态位目前是空的。** `sys1`、`localjev`、`decision-backend-http`各自覆盖了“一个后端→一个Jev格式接口”，但没有一个做了完整的“Jev格式入口 + 多上游运行时切换”。`jev-life`的`DecisionBackend`抽象在架构上最接近，但它被包裹在一个游戏项目中，没有独立产品化。


## 三、战略判断

### 3.1 差异化定位

Jev-Switch的不可替代价值在于：**它是唯一一个“入口是Jev原生格式、出口是多上游”的位置。** 通用网关（LiteLLM、Vercel）的入口是OpenAI/Anthropic格式，TypeSafe官方永远不会提供“切换到Laya本地推理”或“切换到SimpleJev”的能力——因为TypeSafe是闭源商业公司，没有动机帮用户切到自己的替代品。

### 3.2 关键假设与前置验证

在启动编码工作之前，必须先验证以下假设：

**假设A（最关键）：需求存在。** 有足够多的Jev使用者在多个上游之间面临真实的切换痛点，而不是“有当然好”。如果这个假设不成立，后续所有投入都是浪费。

**假设B：你不做，短期内不会有别人做。** 现有项目（sys1、jev-accounts-hub、LLM2Jev）不会在可预见的时间内自然演化出一个统一的多上游Jev网关。

**假设C：你能以可承受的精力/资金投入完成一个“够用”的版本。**

**假设A是前提条件。** 假设B和C都可以在事后验证，唯独假设A如果不主动验证，它永远不会以“阻止你”的方式浮现出来。

### 3.3 7天前置验证实验

**实验内容**：在Jev生态的活跃社区（sys1的GitHub Issues/Discussions、V2EX AI节点、Reddit r/LocalLLaMA）发布一篇帖子，标题为“Jev多上游切换：有人需要吗？还是只有我觉得烦？”。正文描述具体痛点、展示`jev-life`中`DecisionBackend`的架构草图（ASCII图即可），提出两个具体问题：（1）你现在有没有在多个Jev上游之间切换的需求？如果有，你是怎么做的？（2）如果有一个这样的工具，你会在什么场景下用它？

**投入**：2-3小时（写帖1小时，发布0.5小时，7天内累计互动1-1.5小时），零资金成本。

**继续标准**（任意两条同时出现）：
- ≥3条回复描述了**具体的**多上游切换经历
- ≥1条回复提到了**你没想到的**切换场景
- 有人主动提出“我可以帮你测”
- 帖子成为sys1或jev-accounts-hub仓库近期互动最高的讨论之一

**停止标准**（出现任意一条）：
- 3天内实质回复少于2条，且无具体经历描述
- 主流回复是“用LiteLLM pass-through就够了”或“代码里换adapter几行的事”
- 有人指出jev-accounts-hub已经在做这件事，且做得更好

**实验收益**：正面结果带来第一批潜在用户和具体场景，直接指导MVP优先级；负面结果省下一个项目的时间和精力；模糊结果提示需要换更具体的切入点（如只做“Laya↔TypeSafe本地/云端切换”）。

**前置实验未获得正面信号之前，不启动编码工作。**


## 四、MVP范围定义

### 4.1 必须做（MVP）

基于四条原始需求，MVP的最小可行范围：

**M1：Jev兼容HTTP服务器。** 对外暴露`POST /v1/systemone`，接受`{ state, model, questions }`格式的请求，返回`{ model, answers, usage }`格式的响应。类型定义直接参考TypeSafe官方SDK的`Noul`、`Choice`、`Score`三种问题类型。技术栈建议优先选择TypeScript/Node（参考`jev-life`的`llm-broker.ts`的纯函数设计）或Rust（参考`sys1`的axum实现）。

**M2：多上游配置与切换。** 上游至少支持TypeSafe官方、OpenRouter、Vercel AI Gateway三条路径。配置格式为声明式（如YAML/TOML），支持运行时通过管理端点或CLI切换活跃上游。切换粒度至少支持“会话级”（一次启动期间固定一个上游），争取支持“请求级”（每个请求根据配置或简单规则选择上游）。

**M3：Laya作为特殊上游接入。** 将Laya（`convaiinnovations/laya`，421M参数，Apache 2.0，ModernBERT-large backbone）作为本地决策后端接入。Laya的`predict(state, questions)` API与Jev语义对齐，但传输格式需要适配。注意Laya的限制：英文checkpoint仅支持英文输入，中文会静默降级为随机采样；需要Apple Silicon或NVIDIA GPU。

### 4.2 争取做（MVP后）

**M4：LLM→Jev转接服务。** 以`jev-life`的`llm-broker.ts`为参考，构建一个可配置的转接层：定义Jev问题到LLM输出的映射规则（如何从LLM的JSON回包中提取Noul/Choice/Score），以及置信度校准策略。这是需求4的直接实现。

**M5：转接方式的可微调能力。** 暴露映射规则的配置面，允许用户自定义“哪些Jev问题映射到LLM的哪些输出字段”、“如何处理LLM回包中的不确定情况”等。

### 4.3 明确不做

- Agent工具路由（不拦截、不参与工具选择决策）
- Agent配置管理（不管理`~/.claude/settings.json`等Agent配置文件）
- 通用LLM格式入口（不暴露OpenAI/Anthropic兼容的`/v1/chat/completions`或`/v1/messages`端点）
- 流式响应（Jev本身无流式输出）


## 五、技术架构参考

### 5.1 可复用的现有资产

**sys1的Rust实现**：`src/`目录下的axum路由和Serde类型定义可以直接作为M1的参考或起点。它对`/v1/systemone`和`/v1/decide`的双端点支持值得保留。

**jev-life的`DecisionBackend`抽象**：`llm-broker.ts`的纯函数设计（不碰网络、不碰DOM、不读环境变量）是M4的理想参考。发请求由调用方负责，broker只做格式转换，这保证了转接层的可测试性和可移植性。

**LLM2Jev的`llm2jev-serve`**：作为M4的另一个参考实现，它展示了如何将SGLang的HTTP服务器包装成System One兼容API，以及prefill-only binary inference的具体实现路径。

**jev-accounts-hub**：如果其源码公开，应检查其多账户管理和网关分发的实现方式，作为M2的参考。目前仅有描述“A multi-account manager and API gateway for Jev”，具体切换粒度未知。

### 5.2 建议的技术栈

如果优先考虑**快速迭代和与现有Jev生态的互操作性**，建议TypeScript/Node。优势：可以直接参考`jev-life`的broker实现，与TypeSafe官方JavaScript SDK共享类型定义，部署形态灵活（本地CLI或轻量HTTP服务）。

如果优先考虑**性能和单二进制分发**，建议Rust。优势：可以直接 fork 或扩展`sys1`的代码库，利用axum的成熟生态，编译为单二进制便于分发。


## 六、风险与缓解

**风险1：需求验证失败。** 7天实验如果无正面信号，应暂停项目，转而研究jev-accounts-hub或其他已有方案。**缓解**：前置实验作为硬性门槛，实验未通过不启动编码。

**风险2：LiteLLM/Vercel吞并。** 如果通用网关开始提供Jev原生格式的入口，Jev-Switch的差异化价值可能被压缩。**缓解**：持续监控LiteLLM的pass-through文档和Vercel AI Gateway的changelog。如果通用网关开始支持`/typesafe/v1/systemone`的多上游路由，需要重新评估定位。

**风险3：Laya的限制。** 英文checkpoint不支持中文输入，这限制了M3的适用范围。**缓解**：Laya有Router机制可以dispatch到多语言checkpoint，但需要验证其可用性和质量。在M3中明确标注语言支持范围。

**风险4：精力/资金约束。** 发起人已明确表示“过度消耗了精力”，且后期维护能力有限。**缓解**：MVP范围严格限定在M1+M2+M3，M4和M5作为可选扩展。优先选择社区已有代码作为起点（sys1的Rust代码或jev-life的broker实现），减少从零开始的投入。


## 七、明天可以开始的第一个动作

**打开sys1的GitHub Issues区，发一个Discussion**，标题为“有没有人需要在TypeSafe / OpenRouter / Vercel / Laya之间切换Jev上游？”。

正文用写`jev-life` broker时的真实体验开头：“我在写一个Jev兼容的broker时发现，切换上游需要改代码而不是改配置……”然后附上`DecisionBackend`抽象的设计草图（ASCII架构图即可），最后提出两个具体问题：你现在有没有在多个Jev上游之间切换的需求？如果有，你是怎么做的？如果有一个这样的工具，你会在什么场景下用它？

**这件事明天花30分钟就能做完。** 做完之后，7天内不启动任何Jev-Switch的编码工作，等待信号。