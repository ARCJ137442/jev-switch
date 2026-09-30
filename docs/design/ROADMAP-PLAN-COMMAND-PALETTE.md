# 命令面板 Phase 1 计划

**作者**：GPT-6 Luna xhigh（OpenAI Codex）  
**AI 披露**：本文由 AI 协助整理。

**当前状态（2026-09-30）**：导航型命令面板已随 v0.2.0 发布，支持 `Ctrl+K`/`Ctrl+P`、搜索、键盘移动/执行/关闭，并避开输入控件快捷键；UI test/build 门禁已通过。添加入口/提供商等动作注册、常用命令排序和真实桌面快捷键交互仍是后续扩展，不再作为 Phase 1 待实现项。

**目标**：用 `Ctrl+K`/`Ctrl+P` 把页面导航和少量安全动作变成一步可达入口，先做稳定的命令注册表，再扩展动作。

## 现有入口

- `ui/src/app/Shell.tsx`：顶栏导航、hash route 和全局键盘事件适合承载面板。
- `ui/src/App.tsx`：当前五页路由分派。
- Providers/Entries/Routing 页面已有新增动作和可复用 callback，但尚未有统一命令模型。

## 分阶段工作

1. 定义 `Command { id, labelKey, keywords, shortcut?, availability, execute }`，导航命令从现有 route 目录派生。
2. 建立 `CommandPalette`：打开/关闭、输入过滤、最近使用、上下键/Enter/Escape、焦点回收、移动端关闭。
3. Phase 1 只实现页面跳转和无副作用动作：Dashboard、Providers、Entries、Routing DAG、Playground、打开设置。
4. Phase 2 再注册“添加提供商/入口”等动作，并要求前置条件与权限检查；动作执行后关闭面板并显示 toast。
5. 增加快捷键冲突、输入框/文本编辑器不抢快捷键、只读用户过滤管理动作的测试。

## 完成证据

键盘和鼠标均可用；命令排序稳定；窄屏不遮挡内容；只读/未登录状态不会暴露不可执行动作；无重复导航字典。

## 不做

不在第一阶段接入模糊 AI 搜索、不执行不可撤销的删除/覆盖、不引入第三方命令框架。

— GPT-6 Luna xhigh（OpenAI Codex），AI 辅助整理
