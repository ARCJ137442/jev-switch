# Jev-Switch 图标系统设计方案

**日期**: 2026-09-24  
**版本**: 1.0  
**状态**: 设计定稿，待实施  
**作者**: Claude Opus 4.8

---

## §0 设计目标（一句话）

**用标准的图标库（lucide-react）系统化结构化全方位替代 Unicode 字符与 emoji，实现视觉美观与 UI 辅助功能。**

---

## §1 现状分析

### 1.1 当前使用的 Unicode 字符与 emoji

通过代码扫描，发现以下 Unicode 字符与 emoji 需要替换：

| 位置 | 当前字符 | 语义 | 建议替换图标 |
|------|---------|------|-------------|
| **仪表盘** | - | - | - |
| DashboardPage | `◉` | 健康状态圆环 | `<Circle>` (lucide) |
| **路由页面** | - | - | - |
| BipartiteCanvas | `◉` | Pin 端口（实心圆点） | `<Circle fill>` |
| BipartiteCanvas | `○` | Pass 端口（空心圆点） | `<Circle>` |
| RoutingPage | `↗` | 路由箭头数量 | `<TrendingUp>` 或 `<ArrowUpRight>` |
| **演练场** | - | - | - |
| TestPanel | `▼` | 下拉菜单展开 | `<ChevronDown>` |
| **提供商卡片** | - | - | - |
| ProviderCard | `●` | 状态圆点 | `<Circle fill>` + 颜色变体 |
| **模态框与按钮** | - | - | - |
| 通用 | `✓` | 成功/复制成功 | `<Check>` |
| 通用 | `⊗` | 删除 | `<X>` 或 `<Trash2>` |
| 通用 | `✕` | 关闭 | `<X>` |
| 通用 | `⚙` | 设置 | `<Settings>` |

### 1.2 参考项目调研

**参考项目**：
1. **jev-scheme-C.html**（已完成设计）：使用 SVG 图标系统（`.ico` 类）
2. **CC Switch**：大量使用 lucide-react 图标
3. **sub2api**：使用 lucide-react + heroicons 混合
4. **https://user.modelshare.cc/**：使用 lucide-react

**结论**：**lucide-react** 是最广泛使用的图标库，与我们的设计风格高度契合。

---

## §2 图标库选型

### 2.1 候选图标库对比

| 图标库 | 图标数量 | 包大小 | 风格 | React 支持 | 树摇优化 | 推荐度 |
|-------|---------|-------|------|-----------|---------|--------|
| **lucide-react** | 1300+ | ~50KB (tree-shaken) | 简洁现代 | ✅ 原生 | ✅ 优秀 | ⭐⭐⭐⭐⭐ |
| heroicons | 450+ | ~40KB | 圆润 | ✅ 官方 | ✅ 良好 | ⭐⭐⭐⭐ |
| react-icons | 20000+ | ~200KB | 混合 | ✅ 包装 | ❌ 一般 | ⭐⭐⭐ |
| @iconify/react | 100000+ | ~30KB | CDN | ✅ 动态 | ✅ 良好 | ⭐⭐⭐ |

**最终选择**: **lucide-react**

**理由**:
1. ✅ **风格一致**: 线性风格，与 jev-scheme-C.html 的设计高度契合
2. ✅ **树摇优化**: 只打包实际使用的图标，体积小
3. ✅ **React 原生**: TypeScript 类型完整，无需额外封装
4. ✅ **广泛采用**: CC Switch、sub2api、modelshare.cc 都在用
5. ✅ **活跃维护**: Figma 团队维护，更新频繁

---

## §3 图标映射表（完整）

### 3.1 仪表盘 (Dashboard)

| 语义 | 当前 | 替换为 | lucide-react 组件 | 使用场景 |
|------|------|--------|------------------|---------|
| 运行中 | 文字 | `<Activity>` | Activity | daemon 运行状态 |
| 健康圆环 | `◉` | `<Circle>` | Circle | 健康状态圆环（支持 fill 属性） |
| 刷新 | 按钮文字 | `<RefreshCw>` | RefreshCw | 刷新按钮 |
| 模式切换 | 文字 | `<Zap>` | Zap | local/cloud mode 图标 |
| 监听地址 | 文字 | `<Server>` | Server | listen 地址图标 |

### 3.2 提供商 (Providers)

| 语义 | 当前 | 替换为 | lucide-react 组件 | 使用场景 |
|------|------|--------|------------------|---------|
| 添加提供商 | `+ 添加提供商` | `<Plus>` + 文字 | Plus | 添加按钮 |
| 编辑 | 文字 | `<Edit>` | Edit | 编辑按钮 |
| 删除 | `⊗` | `<Trash2>` | Trash2 | 删除按钮 |
| 设置 | `⚙` | `<Settings>` | Settings | 配置按钮 |
| 健康状态 | `●` | `<Circle>` | Circle | 状态圆点（绿/黄/红） |
| 启用/禁用 | 开关 | `<ToggleLeft>` / `<ToggleRight>` | ToggleLeft/Right | 启用开关 |
| Probe 测试 | 按钮文字 | `<Activity>` | Activity | 探测按钮 |
| 复制密钥 | 按钮 | `<Copy>` → `<Check>` | Copy → Check | 复制成功动画 |

### 3.3 路由 (Routing)

| 语义 | 当前 | 替换为 | lucide-react 组件 | 使用场景 |
|------|------|--------|------------------|---------|
| 入口 tab | 文字 | `<Inbox>` | Inbox | 「入口」tab 图标 |
| 路由 tab | 文字 | `<GitBranch>` | GitBranch | 「路由」tab 图标 |
| 新建入口 | `+ 新建入口` | `<Plus>` + 文字 | Plus | 新建入口按钮 |
| Pin 端口 | `◉` | `<Circle fill>` | Circle | 实心圆点（钉死端口） |
| Pass 端口 | `○` | `<Circle>` | Circle | 空心圆点（透传端口） |
| 路由数量 | `2 ↗` | `<TrendingUp>` + 数字 | TrendingUp | 路由数量指示 |
| 编辑 | 文字 | `<Edit>` | Edit | 编辑入口 |
| 删除 | `⊗` | `<Trash2>` | Trash2 | 删除入口 |
| 复制 ID | `✓` | `<Copy>` → `<Check>` | Copy → Check | 复制模型 ID |
| 启用/禁用 | 开关 | `<ToggleLeft>` / `<ToggleRight>` | ToggleLeft/Right | 入口开关 |
| 箭头 priority | 数字圆圈 | 保持现有 SVG | - | 箭头中点圆环图 |
| 添加路由 | `+ 添加路由` | `<Plus>` + 文字 | Plus | 路由管理中添加 |
| 删除路由 | `[X]` | `<X>` | X | 路由管理中删除 |
| 健康圆环 | `◉` | `<Circle>` | Circle | 健康状态（红/黄/绿） |

### 3.4 演练场 (Playground)

| 语义 | 当前 | 替换为 | lucide-react 组件 | 使用场景 |
|------|------|--------|------------------|---------|
| 运行测试 | 按钮文字 | `<Play>` | Play | 运行按钮 |
| 停止 | 按钮文字 | `<Square>` | Square | 停止按钮 |
| 清空 | 按钮文字 | `<RotateCcw>` | RotateCcw | 清空/重置 |
| 下拉菜单 | `▼` | `<ChevronDown>` | ChevronDown | 展开菜单 |
| 示例选择 | 文字 | `<FileText>` | FileText | 示例文件图标 |
| 模型选择 | 文字 | `<Cpu>` | Cpu | 模型图标 |
| 成功 | `✓` | `<Check>` | Check | 成功状态 |
| 失败 | `✕` | `<X>` | X | 失败状态 |
| 加载中 | 文字 | `<Loader2>` | Loader2 | 加载动画（旋转） |

### 3.5 通用组件 (Common)

| 语义 | 当前 | 替换为 | lucide-react 组件 | 使用场景 |
|------|------|--------|------------------|---------|
| 关闭模态框 | `✕` | `<X>` | X | 模态框右上角关闭 |
| 保存 | 按钮文字 | `<Save>` | Save | 保存按钮 |
| 取消 | 按钮文字 | `<X>` | X | 取消按钮 |
| 刷新 | 按钮文字 | `<RefreshCw>` | RefreshCw | 刷新按钮 |
| 信息提示 | 文字 | `<Info>` | Info | 信息图标 |
| 警告 | 文字 | `<AlertTriangle>` | AlertTriangle | 警告图标 |
| 错误 | 文字 | `<AlertCircle>` | AlertCircle | 错误图标 |
| 成功 | `✓` | `<CheckCircle>` | CheckCircle | 成功图标 |
| 帮助 | 文字 | `<HelpCircle>` | HelpCircle | 帮助/问号 |
| 搜索 | 文字 | `<Search>` | Search | 搜索框图标 |
| 设置 | `⚙` | `<Settings>` | Settings | 设置图标 |
| 主题切换 | `☀/☾` | `<Sun>` / `<Moon>` | Sun / Moon | 主题切换 |
| 语言切换 | 文字 | `<Globe>` | Globe | 语言切换 |

---

## §4 实施方案

### 4.1 安装依赖

```bash
npm install lucide-react --save
```

### 4.2 创建图标封装组件

**文件**: `ui/src/components/ui/Icon.tsx`

```tsx
import * as LucideIcons from 'lucide-react';

interface IconProps {
  name: keyof typeof LucideIcons;
  size?: number;
  className?: string;
  color?: string;
  strokeWidth?: number;
}

export function Icon({ name, size = 16, className, color, strokeWidth = 2 }: IconProps) {
  const LucideIcon = LucideIcons[name];
  if (!LucideIcon) {
    console.warn(`Icon "${name}" not found in lucide-react`);
    return null;
  }
  return <LucideIcon size={size} className={className} color={color} strokeWidth={strokeWidth} />;
}

// 预设尺寸
export const IconSize = {
  xs: 12,
  sm: 14,
  md: 16,
  lg: 20,
  xl: 24,
} as const;
```

### 4.3 创建状态圆点组件

**文件**: `ui/src/components/ui/StatusDot.tsx`

```tsx
import { Circle } from 'lucide-react';

type Status = 'healthy' | 'degraded' | 'failed' | 'idle';

const statusColors = {
  healthy: 'text-green-600',
  degraded: 'text-yellow-600',
  failed: 'text-red-600',
  idle: 'text-gray-400',
};

interface StatusDotProps {
  status: Status;
  size?: number;
  fill?: boolean;
}

export function StatusDot({ status, size = 8, fill = true }: StatusDotProps) {
  return (
    <Circle
      size={size}
      className={statusColors[status]}
      fill={fill ? 'currentColor' : 'none'}
      strokeWidth={fill ? 0 : 2}
    />
  );
}
```

### 4.4 创建复制按钮组件

**文件**: `ui/src/components/ui/CopyButton.tsx`

```tsx
import { useState } from 'react';
import { Copy, Check } from 'lucide-react';

interface CopyButtonProps {
  text: string;
  className?: string;
  size?: number;
}

export function CopyButton({ text, className, size = 16 }: CopyButtonProps) {
  const [copied, setCopied] = useState(false);

  const handleCopy = async () => {
    await navigator.clipboard.writeText(text);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  return (
    <button
      onClick={handleCopy}
      className={className}
      title="复制到剪贴板"
      aria-label="复制到剪贴板"
    >
      {copied ? <Check size={size} className="text-green-600" /> : <Copy size={size} />}
    </button>
  );
}
```

---

## §5 迁移清单

### 5.1 优先级 P0（核心功能）

| 组件 | 替换项 | 工作量 |
|------|-------|-------|
| Shell.tsx | `☀/☾` → `<Sun>/<Moon>` | 0.5h |
| Shell.tsx | `⚙` → `<Settings>` | 0.5h |
| DashboardPage.tsx | `◉` → `<StatusDot>` | 1h |
| ProviderCard.tsx | `●` → `<StatusDot>` | 1h |
| ProviderCard.tsx | 按钮图标（编辑/删除/设置） | 1h |
| BipartiteCanvas.tsx | `◉/○` → `<Circle>` | 1.5h |
| RoutingPage.tsx | `↗` → `<TrendingUp>` | 0.5h |
| TestPanel.tsx | `▼` → `<ChevronDown>` | 0.5h |
| TestPanel.tsx | 运行/停止按钮图标 | 0.5h |

**总计**: ~7 小时

### 5.2 优先级 P1（增强体验）

| 组件 | 替换项 | 工作量 |
|------|-------|-------|
| 服务入口卡片 | 完整图标系统（编辑/删除/复制/启用） | 2h |
| 编辑模态框 | 完整图标系统（关闭/保存/取消/添加/删除） | 2h |
| ConflictBanner | `<AlertTriangle>` 警告图标 | 0.5h |
| 通用按钮 | 统一图标风格 | 1.5h |

**总计**: ~6 小时

---

## §6 设计规范

### 6.1 图标尺寸规范

| 场景 | 尺寸 | 示例 |
|------|------|------|
| 按钮图标 | 16px | 编辑、删除、复制按钮 |
| 导航图标 | 18px | 顶栏导航 tab |
| 状态圆点 | 8px | 健康状态 |
| 卡片标题图标 | 20px | provider 卡片标题 |
| 大图标 | 24px | 空状态占位 |

### 6.2 图标颜色规范

| 状态 | 颜色 | Tailwind 类 |
|------|------|------------|
| 正常 | `var(--ink)` | `text-ink` |
| 主色 | `var(--primary)` | `text-primary` |
| 成功 | `#047857` | `text-green-700` |
| 警告 | `#b45309` | `text-yellow-700` |
| 错误 | `#b91c1c` | `text-red-700` |
| 禁用 | `var(--muted)` | `text-muted` |

### 6.3 图标 strokeWidth 规范

| 场景 | strokeWidth | 说明 |
|------|------------|------|
| 默认 | 2 | 正常图标 |
| 细线 | 1.5 | 大尺寸图标（24px+） |
| 粗线 | 2.5 | 强调图标 |

---

## §7 验收清单

### 7.1 功能验收

- [ ] 所有 Unicode 字符 / emoji 已替换为 lucide-react 图标
- [ ] 主题切换图标（Sun/Moon）正常工作
- [ ] 状态圆点（健康/降级/失败）颜色正确
- [ ] 复制按钮动画（Copy → Check）流畅
- [ ] 下拉菜单展开图标（ChevronDown）正确旋转
- [ ] 所有按钮图标对齐居中

### 7.2 视觉验收

- [ ] 图标尺寸统一（16px 按钮图标）
- [ ] 图标颜色符合设计规范
- [ ] 图标 strokeWidth 一致（2px）
- [ ] 图标与文字垂直对齐
- [ ] hover 状态图标颜色变化流畅

### 7.3 性能验收

- [ ] 打包体积增加 < 50KB（树摇优化）
- [ ] 首屏加载无明显延迟
- [ ] 图标渲染无闪烁

---

## §8 下一步

1. **立即**: 安装 lucide-react 依赖
2. **Phase 1**: 创建基础组件（Icon / StatusDot / CopyButton）
3. **Phase 2**: 迁移 P0 组件（Shell / Dashboard / Providers / Routing）
4. **Phase 3**: 迁移 P1 组件（服务入口 / 模态框 / 通用按钮）
5. **Phase 4**: 验收测试 + 截图更新 README

---

**文档作者**: Claude Opus 4.8  
**最后更新**: 2026-09-24  
**下一步**: 安装 lucide-react 并创建基础图标组件
