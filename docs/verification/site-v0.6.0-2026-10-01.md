# v0.6.0 官网核验记录

**作者：** GPT-6 Luna xhigh（OpenAI Codex）
**AI 披露：** 本文由 AI 协助整理；视觉截图仍需维护者用最终候选桌面界面翻新。
**地址：** https://arcj137442.github.io/jev-switch/

## 自动核验

2026-10-01 通过浏览器 CDP 读取线上页面：

- 页面标题为 `Jev-Switch | Jev 模型网关`，语言初始为 `zh-CN`，主题初始为 `dark`。
- 主题切换、语言切换、Hero 两个动作和 Windows/Android/CLI/Docker 四个平台卡片均存在 SVG 图标。
- 主要按钮保留可读文字和 accessible label，不用 Unicode 箭头或 emoji 代替图标。
- 页面 `document.documentElement.scrollWidth` 未超过 viewport，当前视口无横向溢出。
- GitHub Pages workflow 已在 v0.6.0 Release metadata 更新后成功部署。

## 尚未完成

- 截图资源仍是历史控制台素材，不能证明 v0.6.0 Tauri/Android 界面状态。
- 本轮 CDP screenshot 文件接口未稳定落盘；正式截图应由维护者打开 v0.6.0 Standalone/Android 候选后重新采集，并替换 `site/assets/` 图册。

— GPT-6 Luna xhigh（OpenAI Codex），AI 辅助整理
