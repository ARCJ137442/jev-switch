# 跟随系统主题计划

**作者**：GPT-6 Luna xhigh（OpenAI Codex）  
**AI 披露**：本文由 AI 协助整理。

**目标**：在保留手动浅色/深色切换的同时增加“跟随系统”，让系统主题变化能自动同步，并支持当前已有的深浅色视觉回归。

## 现有入口

- `ui/src/main.tsx`：首帧读取 `localStorage['jev_theme']`，无值时读取 `prefers-color-scheme`。
- `ui/src/components/ui/ThemeToggle.tsx`：当前 `light/dark` 两态切换。
- `ui/src/styles/tokens.css`：`[data-theme='dark']` 主题变量与 reduced-motion 规则。

## 分阶段工作

1. 将设置值从 `light|dark` 扩为 `system|light|dark`，旧值无迁移成本地继续有效。
2. 在 `main.tsx` 集中解析有效主题，注册 `matchMedia('(prefers-color-scheme: dark)')` 监听，仅在 `system` 时更新 `data-theme`。
3. 把 ThemeToggle 改成图标+菜单/分段选择，明确显示当前模式和 tooltip；不复制主题状态源。
4. 增加 light/dark/system、系统切换、刷新恢复、无 localStorage 四组测试；跑五页和短窗口截图回归。

## 完成证据

用户选择 `system` 后切换系统主题，页面无需刷新即可改变；手动 `light/dark` 不被系统覆盖；刷新后选择保留。

## 不做

不在此计划加入新的颜色体系、动效重做、字体重构或第三方主题库。

— GPT-6 Luna xhigh（OpenAI Codex），AI 辅助整理
