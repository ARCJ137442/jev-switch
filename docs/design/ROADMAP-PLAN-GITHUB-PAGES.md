# GitHub Pages 官方首页计划

**作者**：GPT-6 Luna xhigh（OpenAI Codex）  
**AI 披露**：本文由 AI 协助整理。

**当前状态（2026-09-30）**：纯静态首页与 Pages workflow 已于 v0.2.0 时部署，桌面 DOM 内容已核验。v0.4.0 发布后，本地工作树中的站点文案已同步当前版本、CLI 入口和“尚未经人工实测”边界；Pages 部署和移动端正式截图仍待后续发布窗口核验。

**目标**：建立一个无需自有域名、与当前 Release 和 README 一致的静态首页，让新用户先看到产品定位、真实界面和限制。

## 现有入口

- 根 `README.md`：定位、截图、下载和文档入口。
- `docs/screenshots.md` 与 `docs/images/`：已筛选的公开界面素材。
- `.github/workflows/`：目前只有 CI/Release，需要新增 Pages workflow。

## 分阶段工作

1. 选择静态构建方式（优先纯 HTML/CSS/少量 JS 或现有 UI 的静态子项目），避免复制 React 控制台和引入第二套产品真相。
2. 首页首屏写一句话定位、真实 Dashboard/路由/Playground 图，第二屏写快速开始、Windows 下载、Docker 入口和当前限制。
3. 所有下载链接从当前 Release 变量或明确的 Release URL 派生；显示“TypeSafe 已支持、OpenRouter 为上游转换 adapter、Windows 优先”等真实边界。
4. 添加 GitHub Pages workflow：构建、预览、链接检查、移动视口检查；部署到仓库 Pages，必要时再更新 GitHub About Website。

## 完成证据

- Pages 地址可访问，桌面/移动视口无溢出。
- 首页文字、截图、版本和 Release 附件一致。
- CI 能从干净 checkout 构建，页面不含 key、AppData 或本地运行数据。

## 不做

不在这项计划里实现域名、在线控制台、下载统计、营销动画或与仓库 README 分叉的功能说明。

— GPT-6 Luna xhigh（OpenAI Codex），AI 辅助整理
