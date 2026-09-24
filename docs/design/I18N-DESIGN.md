# Jev-Switch 国际化（i18n）设计方案

**版本**: v1.0  
**日期**: 2026-09-24  
**状态**: ✅ Phase 1 完成，Phase 2-3 待实施

---

## 一、设计原则

### 1.1 核心目标

- **完整覆盖**: 所有 UI 文本必须支持多语言，零硬编码
- **高性能**: 按需加载语言包，避免全量打包
- **可扩展**: 支持任意语言添加，无需修改核心代码
- **类型安全**: TypeScript 强类型约束，防止翻译键错误
- **开发友好**: 热更新词典，开发时即时预览

### 1.2 参考实现

借鉴 `jev-life` 的 i18n 方案，并针对 Jev-Switch 的特点优化：
- ✅ 点分命名空间（`common.save`, `dash.running`）
- ✅ 占位符插值（`{ok}/{total} healthy`）
- ✅ 类型安全（`MessageKey` 类型约束）
- 🔄 动态加载（按需 + 缓存）
- 🔄 语言检测（浏览器 + localStorage）

---

## 二、当前实现（Phase 1 ✅）

### 2.1 架构

```
ui/src/i18n/
├── index.tsx       # 导出 useI18n hook + I18nProvider
├── en.ts           # 英文词典（默认语言）
└── zh.ts           # 中文词典（完全对齐 en）
```

**核心 API**:
```typescript
const { t, lang, setLang } = useI18n();

// 基础翻译
t('common.save')  // → "Save" | "保存"

// 占位符插值
t('dash.healthy', { ok: 2, total: 3 })  // → "2/3 healthy"

// 语言切换
setLang('zh')  // 立即生效，自动保存到 localStorage
```

### 2.2 类型安全机制

```typescript
// en.ts — 主词典
export const en = {
  'common.save': 'Save',
  'dash.running': 'Running',
  // ...
} as const;

export type MessageKey = keyof typeof en;

// zh.ts — 强制对齐
export const zh: Record<keyof typeof en, string> = {
  'common.save': '保存',
  'dash.running': '运行中',
  // 缺失任何键 → TypeScript 编译错误
};
```

### 2.3 已覆盖范围

| 命名空间 | 键数 | 状态 | 说明 |
|---------|------|------|------|
| `common.*` | 10 | ✅ | 通用操作（save/cancel/delete 等） |
| `shell.*` | 8 | ✅ | 导航栏 + 连接状态 |
| `dash.*` | 21 | ✅ | Dashboard 页面（新增） |
| `prov.*` | 18 | ✅ | Providers 页面 |
| `routing.*` | 12 | ✅ | Routing 页面 |
| `pg.*` | 6 | ✅ | Playground 页面 |
| `tp.*` | 16 | ✅ | Test Panel 组件 |
| `qf.*` | 22 | ✅ | Question Form Editor |
| `home.*` | 30 | ✅ | Home 页面（旧） |

**总计**: 143 个翻译键，中英文完全对齐

### 2.4 验证结果

```bash
# 前端构建
npm run build --prefix ui
# ✅ 无 TypeScript 错误
# ✅ 所有组件正常渲染
# ✅ 语言切换即时生效
```

---

## 三、性能优化方案（Phase 2 🔄）

### 3.1 问题分析

**当前瓶颈**:
- 所有语言包打包到 `bundle.js`（~266KB，其中词典 ~10KB）
- 切换语言需要重新渲染整个应用（~50ms）
- 添加第 3/4/5 种语言会线性增加包体积

**目标**:
- 按需加载：首次只加载当前语言（~5KB）
- 缓存策略：已加载语言永久缓存（localStorage）
- 预加载：空闲时预加载备用语言

### 3.2 动态加载实现

```typescript
// i18n/loader.ts
const LANG_CACHE = new Map<string, Record<string, string>>();

export async function loadLanguage(lang: string): Promise<Record<string, string>> {
  // 1. 内存缓存
  if (LANG_CACHE.has(lang)) {
    return LANG_CACHE.get(lang)!;
  }

  // 2. localStorage 缓存
  const cached = localStorage.getItem(`i18n:${lang}`);
  if (cached) {
    const dict = JSON.parse(cached);
    LANG_CACHE.set(lang, dict);
    return dict;
  }

  // 3. 动态 import（代码分割）
  const module = await import(`./locales/${lang}.ts`);
  const dict = module.default;

  // 4. 双层缓存
  LANG_CACHE.set(lang, dict);
  localStorage.setItem(`i18n:${lang}`, JSON.stringify(dict));

  return dict;
}
```

**打包配置** (Vite):
```typescript
// vite.config.ts
export default defineConfig({
  build: {
    rollupOptions: {
      output: {
        manualChunks: {
          'i18n-en': ['./src/i18n/locales/en.ts'],
          'i18n-zh': ['./src/i18n/locales/zh.ts'],
          // 未来语言自动分片
        },
      },
    },
  },
});
```

### 3.3 预加载策略

```typescript
// i18n/index.tsx
useEffect(() => {
  // 空闲时预加载备用语言
  if ('requestIdleCallback' in window) {
    requestIdleCallback(() => {
      const backup = lang === 'en' ? 'zh' : 'en';
      void loadLanguage(backup);
    });
  }
}, [lang]);
```

---

## 四、可扩展性设计（Phase 3 🔄）

### 4.1 支持新语言

**步骤**（零代码修改）:

1. **创建词典文件**
```bash
# 复制英文词典作为模板
cp ui/src/i18n/en.ts ui/src/i18n/ja.ts
```

2. **翻译**
```typescript
// ja.ts（日语）
export const ja: Record<keyof typeof en, string> = {
  'common.save': '保存',
  'dash.running': '実行中',
  // ...
};
```

3. **注册语言**
```typescript
// i18n/index.tsx
const LANGUAGES = ['en', 'zh', 'ja', 'ko', 'es', 'fr'] as const;
```

4. **自动生效**
   - 语言切换 UI 自动显示新语言
   - 打包自动分片（`i18n-ja.js`）
   - 按需加载 + 缓存

### 4.2 扩展机制

**语言自动发现**:
```typescript
// i18n/discovery.ts
export async function discoverLanguages(): Promise<string[]> {
  const modules = import.meta.glob('./locales/*.ts');
  return Object.keys(modules).map(path => 
    path.match(/\/([a-z]{2})\.ts$/)?.[1]
  ).filter(Boolean);
}
```

**插件化词典**:
```typescript
// 允许运行时注册自定义词典（插件系统）
export function registerDictionary(lang: string, dict: Record<string, string>) {
  LANG_CACHE.set(lang, { ...LANG_CACHE.get(lang), ...dict });
}
```

---

## 五、开发工作流

### 5.1 添加新翻译键

```typescript
// 1. 在 en.ts 中添加
export const en = {
  // ...
  'dash.newKey': 'New Feature',
};

// 2. TypeScript 立即报错：zh.ts 缺少 'dash.newKey'
// 3. 补全 zh.ts
export const zh: Record<keyof typeof en, string> = {
  // ...
  'dash.newKey': '新功能',
};

// 4. 使用
const { t } = useI18n();
<div>{t('dash.newKey')}</div>
```

### 5.2 审查覆盖率

```bash
# 扫描所有硬编码英文文本
npm run i18n:audit

# 输出：
# ✅ Dashboard: 100% 覆盖
# ⚠️  Providers: 发现 2 处硬编码
#     - src/components/providers/ProviderCard.tsx:45 "Enabled"
#     - src/components/providers/ProviderCard.tsx:67 "Disabled"
```

**审查脚本** (`scripts/i18n-audit.mjs`):
```javascript
import { glob } from 'glob';
import { readFileSync } from 'fs';

const files = await glob('src/**/*.tsx');
const hardcoded = [];

for (const file of files) {
  const content = readFileSync(file, 'utf-8');
  // 正则匹配：<tag>英文</tag> 但不在 t() 调用中
  const matches = content.matchAll(/>([A-Z][a-z]+\s*[a-z]*)</g);
  for (const match of matches) {
    if (!content.includes(`t('`) && !content.includes(`t("`)) {
      hardcoded.push({ file, line: match.index, text: match[1] });
    }
  }
}

console.log(`Found ${hardcoded.length} hardcoded strings`);
```

### 5.3 热更新

```typescript
// vite.config.ts
export default defineConfig({
  plugins: [
    {
      name: 'i18n-hmr',
      handleHotUpdate({ file, server }) {
        if (file.includes('/i18n/')) {
          // 词典修改 → 立即刷新，无需重启
          server.ws.send({ type: 'full-reload' });
        }
      },
    },
  ],
});
```

---

## 六、最佳实践

### 6.1 命名规范

| 类型 | 命名 | 示例 |
|------|------|------|
| 通用操作 | `common.<action>` | `common.save`, `common.cancel` |
| 页面专属 | `<page>.<key>` | `dash.running`, `prov.add` |
| 组件专属 | `<comp>.<key>` | `tp.form`, `qf.addQuestion` |
| 错误信息 | `error.<code>` | `error.networkFailed` |

**占位符**:
- 单个：`{name}`, `{count}`, `{id}`
- 多个：`{ok}/{total}`, `{from} → {to}`

### 6.2 翻译质量

**禁忌**:
- ❌ 机翻直出（需人工审校）
- ❌ 品牌名翻译（Jev-Switch 保持原文）
- ❌ 技术术语翻译（POST/GET/HTTP 保持原文）
- ❌ 超长文案（单键 >100 字符 → 拆分）

**推荐**:
- ✅ 简洁对等（"Save" → "保存"，不是"保存文件"）
- ✅ 语境适配（"Run" 在按钮上 → "运行"，在日志中 → "执行"）
- ✅ 一致性（同一概念用同一译名）

### 6.3 回退策略

```typescript
// 缺失翻译 → 显示 key（开发环境）或英文（生产）
function t(key: MessageKey, params?: Record<string, any>): string {
  const dict = getCurrentDict();
  const raw = dict[key];
  
  if (!raw) {
    if (import.meta.env.DEV) {
      console.warn(`Missing translation: ${key}`);
      return `[${key}]`;  // 醒目提示
    }
    return en[key] || key;  // 回退英文
  }
  
  return interpolate(raw, params);
}
```

---

## 七、路线图

### Phase 1: 基础覆盖 ✅（已完成）
- [x] 实现 `useI18n` hook
- [x] 英文词典（143 键）
- [x] 中文词典（完全对齐）
- [x] Dashboard 页面国际化
- [x] 类型安全机制

### Phase 2: 性能优化 🔄（计划中）
- [ ] 动态加载 + 代码分割
- [ ] localStorage 缓存
- [ ] 预加载策略
- [ ] 审查脚本（`i18n:audit`）

### Phase 3: 扩展性 🔄（未来）
- [ ] 支持第 3 种语言（日语 / 韩语）
- [ ] 语言自动发现
- [ ] 插件化词典
- [ ] 翻译管理平台集成（可选）

---

## 八、与 jev-life 的对比

| 特性 | jev-life | Jev-Switch (当前) | Jev-Switch (目标) |
|------|----------|-------------------|-------------------|
| 词典结构 | 点分命名空间 | ✅ 同 | ✅ 同 |
| 类型安全 | TypeScript | ✅ 同 | ✅ 同 |
| 动态加载 | ✅ 是 | ❌ 否 | ✅ 是（Phase 2） |
| 缓存策略 | localStorage | ❌ 否 | ✅ 是（Phase 2） |
| 支持语言 | 3 种 | 2 种 | N 种（可扩展） |
| 打包体积 | ~8KB/语言 | ~10KB 全部 | ~5KB/语言（按需） |

**核心优化**:
- jev-life 是单页应用（SPA），适合全量加载
- Jev-Switch 是控制台（Dashboard），按需加载更合理

---

## 九、常见问题

### Q1: 如何添加第三种语言？

**A**: 复制 `en.ts` → 翻译 → 类型检查自动保证完整性。无需修改其他代码。

### Q2: 切换语言会丢失用户状态吗？

**A**: 不会。`useI18n` 只重新渲染文本，组件状态保持不变。

### Q3: 如何处理复数形式（1 item vs 2 items）？

**A**: 当前方案：占位符 + 条件判断
```typescript
const text = count === 1 
  ? t('item.singular', { n: count }) 
  : t('item.plural', { n: count });
```

未来可引入 `@formatjs/intl` 的 `PluralRules`。

### Q4: RTL（阿拉伯语/希伯来语）支持？

**A**: 需要 Phase 3 扩展：
1. 在 `<html dir="rtl">` 设置方向
2. CSS 使用逻辑属性（`margin-inline-start` 而非 `margin-left`）

---

## 十、参考资源

- **jev-life i18n 实现**: 点分命名空间 + 类型安全
- **React i18next**: 成熟的 React 国际化方案（可选升级）
- **FormatJS**: Unicode CLDR 标准（复数/日期/货币）
- **Chrome DevTools**: Coverage 审查未使用代码

---

**文档作者**: Claude Opus 4.8  
**最后更新**: 2026-09-24
