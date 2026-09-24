# Jev-Switch 国际化使用指南

**版本**: v1.0  
**日期**: 2026-09-24  
**目标读者**: 开发者、贡献者

---

## 快速开始

### 前端切换语言

**方式 1: UI 切换**
- 点击右上角语言按钮（EN / 中）
- 语言立即切换，自动保存到 localStorage

**方式 2: URL 参数**（开发调试）
```
http://localhost:11435/?lang=zh
http://localhost:11435/?lang=en
```

**方式 3: 浏览器控制台**
```javascript
localStorage.setItem('jev-switch-lang', 'zh');
location.reload();
```

---

## 当前状态

### 支持的语言

| 语言 | 代码 | 完成度 | 翻译键数 |
|------|------|--------|---------|
| English | `en` | ✅ 100% | 164 |
| 中文 | `zh` | ✅ 100% | 164 |

### 覆盖范围

**所有 UI 文本已完成国际化**（Phase 1 ✅）：

| 页面/组件 | 翻译键数 | 示例键 |
|----------|---------|--------|
| Dashboard | 21 | `dash.running`, `dash.healthy` |
| Providers | 18 | `prov.add`, `card.healthy` |
| Routing | 12 | `routing.edges`, `canvas.dragTitle` |
| Playground | 6 | `pg.quickStart`, `pg.examples` |
| Test Panel | 16 | `tp.form`, `tp.running` |
| Question Form | 22 | `qf.addQuestion`, `qf.typeChoice` |
| Shell/Common | 18 | `common.save`, `shell.navDashboard` |
| Home（旧） | 30 | `home.mode`, `home.uptime` |

**验证方式**:
```bash
cd ui
npm run build  # 无 TypeScript 错误 = 国际化完整
```

---

## 开发者指南

### 添加新翻译键

**步骤**（TypeScript 强制类型安全）：

1. **在 `en.ts` 中添加**
```typescript
// ui/src/i18n/en.ts
export const en = {
  // ... 现有键
  'dash.newFeature': 'New Feature',
} as const;
```

2. **TypeScript 立即报错**
```
❌ Type error: Property 'dash.newFeature' is missing in type ...
   File: ui/src/i18n/zh.ts
```

3. **补全 `zh.ts`**
```typescript
// ui/src/i18n/zh.ts
export const zh: Record<keyof typeof en, string> = {
  // ... 现有键
  'dash.newFeature': '新功能',
};
```

4. **在组件中使用**
```tsx
import { useI18n } from '../i18n';

export function MyComponent() {
  const { t } = useI18n();
  return <div>{t('dash.newFeature')}</div>;
}
```

### 占位符插值

**语法**: `{name}` 占位符

```typescript
// 定义
'dash.healthy': '{ok}/{total} healthy',

// 使用
t('dash.healthy', { ok: 2, total: 3 })
// → "2/3 healthy" (英文)
// → "2/3 健康" (中文)
```

**多个占位符**:
```typescript
'home.rebindOk': 'mode → {mode} · rebind {from} → {to} ok',

t('home.rebindOk', { mode: 'cloud', from: '127.0.0.1:11435', to: '0.0.0.0:11435' })
// → "mode → cloud · rebind 127.0.0.1:11435 → 0.0.0.0:11435 ok"
```

### 命名规范

| 类型 | 命名模式 | 示例 |
|------|---------|------|
| 通用操作 | `common.<action>` | `common.save`, `common.cancel` |
| 页面专属 | `<page>.<key>` | `dash.running`, `prov.add` |
| 组件专属 | `<comp>.<key>` | `tp.form`, `qf.addQuestion` |

**占位符命名**:
- 单个: `{name}`, `{count}`, `{id}`
- 多个: `{ok}/{total}`, `{from} → {to}`

---

## 翻译质量标准

### ✅ 推荐做法

1. **简洁对等**
   - ✅ "Save" → "保存"
   - ❌ "Save" → "保存文件到磁盘"

2. **保留品牌/术语**
   - ✅ Jev-Switch, HTTP, POST, GET（不翻译）
   - ❌ "Jev-Switch" → "杰夫-切换器"

3. **语境适配**
   - "Run" 在按钮上 → "运行"
   - "Run" 在日志中 → "执行"

4. **一致性**
   - 同一概念用同一译名
   - "provider" 始终译为 "提供商"，不混用 "提供者"

### ❌ 禁忌

1. **机翻直出**（需人工审校）
2. **超长文案**（单键 >100 字符需拆分）
3. **技术术语翻译**（POST/GET/JSON 保持原文）

---

## 常见问题

### Q1: 如何审查未翻译的文本？

**A**: 使用前端构建验证
```bash
npm run build --prefix ui
# TypeScript 错误 = 有缺失的翻译键
```

**手动审查**:
1. 切换到中文模式
2. 逐页检查是否有英文残留
3. 发现硬编码 → 提取到 i18n 词典

### Q2: 切换语言后页面没变化？

**A**: 检查三个可能原因：
1. **浏览器缓存** → 强制刷新（Ctrl+Shift+R）
2. **localStorage 未生效** → 打开控制台检查 `localStorage.getItem('jev-switch-lang')`
3. **构建未更新** → `npm run build --prefix ui`

### Q3: 如何添加第三种语言（如日语）？

**A**: Phase 2 计划（`docs/design/I18N-DESIGN.md` §四）
1. 复制 `en.ts` → `ja.ts`
2. 翻译所有键（TypeScript 强制完整性）
3. 注册到 `i18n/index.tsx`
4. 自动生效（无需修改组件）

### Q4: 占位符不生效？

**A**: 检查调用时是否传递了参数对象
```typescript
// ❌ 错误：缺少参数对象
t('dash.healthy')  // → "{ok}/{total} healthy"

// ✅ 正确：传递参数对象
t('dash.healthy', { ok: 2, total: 3 })  // → "2/3 healthy"
```

---

## 架构说明

### 文件结构

```
ui/src/i18n/
├── index.tsx       # 导出 useI18n hook + I18nProvider
├── en.ts           # 英文词典（主词典，164 键）
└── zh.ts           # 中文词典（完全对齐 en.ts）
```

### 类型安全机制

```typescript
// en.ts — 主词典，as const 锁定类型
export const en = {
  'common.save': 'Save',
  // ...
} as const;

export type MessageKey = keyof typeof en;

// zh.ts — 强制对齐，缺少任何键会编译错误
export const zh: Record<keyof typeof en, string> = {
  'common.save': '保存',
  // 缺少 'common.save' → ❌ TypeScript 错误
};
```

### useI18n API

```typescript
const { t, lang, setLang } = useI18n();

// t(key: MessageKey, params?: Record<string, any>): string
t('common.save')  // → "Save" | "保存"
t('dash.healthy', { ok: 2, total: 3 })  // → "2/3 healthy"

// lang: 'en' | 'zh'
console.log(lang);  // 当前语言

// setLang(newLang: 'en' | 'zh'): void
setLang('zh');  // 立即切换到中文，自动保存到 localStorage
```

---

## 路线图

### Phase 1: 基础覆盖 ✅（已完成）
- [x] 实现 `useI18n` hook
- [x] 英文词典（164 键）
- [x] 中文词典（完全对齐）
- [x] 所有页面国际化
- [x] 类型安全机制

### Phase 2: 性能优化 🔄（计划中）
- [ ] 动态加载 + 代码分割（按需加载语言包）
- [ ] localStorage 缓存
- [ ] 预加载策略
- [ ] 审查脚本（`npm run i18n:audit`）

### Phase 3: 扩展性 🔄（未来）
- [ ] 支持第 3 种语言（日语 / 韩语）
- [ ] 语言自动发现
- [ ] 插件化词典
- [ ] 翻译管理平台集成（可选）

详见 `docs/design/I18N-DESIGN.md`

---

## 相关文档

- **设计方案**: `docs/design/I18N-DESIGN.md` — 完整的三阶段设计（性能优化 + 可扩展性）
- **词典文件**: `ui/src/i18n/en.ts`, `ui/src/i18n/zh.ts`
- **用户旅程**: `docs/USER-JOURNEYS.md` §五 — 多语言切换验收

---

## 验收清单

**开发者验收**:
- [ ] `npm run build --prefix ui` 无 TypeScript 错误
- [ ] 所有新增 UI 文本已提取到 i18n 词典
- [ ] 中英文词典键集完全一致

**用户验收**:
1. 打开 http://127.0.0.1:11435
2. 点击右上角语言切换按钮（EN / 中）
3. 验证所有页面文本立即切换
4. 验证占位符插值正常（如 "2/3 healthy" → "2/3 健康"）
5. 刷新页面，语言选择保持

---

**文档作者**: Claude Opus 4.8  
**最后更新**: 2026-09-24
