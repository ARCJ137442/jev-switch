# 国际化 (i18n) 设计方案

**日期**: 2026-09-24  
**版本**: 1.0  
**状态**: 实施中  
**作者**: Claude Opus 4.8

---

## §1 当前实现

### 1.1 架构

**词典文件**:
- `ui/src/i18n/en.ts` — 英文词典（默认语言，CDP 断言依赖）
- `ui/src/i18n/zh.ts` — 中文词典（类型强制与 en 键集一致）

**类型安全**:
```typescript
// en.ts
export const en = {
  'common.reload': 'Reload',
  'shell.navHome': 'Home',
  // ...
} as const;

export type MessageKey = keyof typeof en;

// zh.ts
import type { en } from './en';
export const zh: Record<keyof typeof en, string> = {
  'common.reload': '重载',
  'shell.navHome': '首页',
  // ...
};
```

**使用方式**:
```typescript
import { useI18n } from '@/hooks/useI18n';

function MyComponent() {
  const t = useI18n();
  return <div>{t('common.reload')}</div>;
}
```

### 1.2 命名规范

**格式**: `namespace.key`

| 命名空间 | 用途 | 示例 |
|---------|------|------|
| `common.*` | 通用词汇（按钮、状态） | `common.save`, `common.cancel` |
| `shell.*` | 顶栏导航 | `shell.navHome`, `shell.navRouting` |
| `prov.*` | Providers 页面 | `prov.title`, `prov.add` |
| `routing.*` | Routing 页面 | `routing.edges`, `routing.synced` |
| `pg.*` | Playground 页面 | `pg.heroLead`, `pg.examples` |
| `tp.*` | TestPanel 组件 | `tp.model`, `tp.running` |
| `dash.*` | Dashboard 页面 | `dash.running`, `dash.localMode` |
| `home.*` | Home 页面 | `home.uptime`, `home.mode` |

### 1.3 品牌术语不翻译

以下术语在所有语言中保持英文：
- 品牌名：`Jev-Switch`, `Run Jev`
- HTTP 术语：`POST`, `GET`, `PUT`, `DELETE`
- 技术标识符：`qid`, `key`, `UNSAVED`

---

## §2 测试保障

### 2.1 i18n 键集一致性测试

**测试文件**: `ui/src/tests/i18n.test.ts`

**测试用例**:
1. ✅ zh 和 en 的键集完全一致
2. ✅ zh 没有缺失 en 中的键
3. ✅ zh 没有多余的键
4. ✅ en 没有空值
5. ✅ zh 没有空值
6. ✅ 所有键遵循命名规范（`namespace.key`）

**CI 集成**:
```bash
npm test -- i18n.test.ts
```

---

## §3 多语言扩展设计

### 3.1 目标语言优先级

| 优先级 | 语言 | 原因 |
|-------|------|------|
| P0 | 英文（en） | 默认语言，国际用户 |
| P0 | 中文（zh） | 主要用户群体 |
| P1 | 日文（ja） | 潜在用户群 |
| P1 | 韩文（ko） | 潜在用户群 |
| P2 | 法文（fr） | 欧洲用户 |
| P2 | 德文（de） | 欧洲用户 |

### 3.2 语言切换键设计

**方案 A：ISO 639-1 两字母代码**（推荐）
```typescript
type Locale = 'en' | 'zh' | 'ja' | 'ko' | 'fr' | 'de';
```

**优点**:
- 标准化，易于理解
- URL 路径友好（`/en/home`, `/zh/home`）
- 与浏览器 `navigator.language` 兼容

**方案 B：扩展代码（支持地区变体）**
```typescript
type Locale = 'en-US' | 'en-GB' | 'zh-CN' | 'zh-TW' | 'ja-JP' | 'ko-KR';
```

**优点**:
- 支持地区差异（简体 vs 繁体中文）
- 更精确的本地化

**缺点**:
- 复杂度增加
- 维护成本高

**建议**: 
- **当前阶段使用方案 A**（`en` / `zh`）
- **未来需要时升级到方案 B**（`zh-CN` / `zh-TW`）

### 3.3 语言检测与回退

**优先级**:
1. 用户手动选择（localStorage: `jev-switch-locale`）
2. 浏览器语言（`navigator.language`）
3. 默认英文（`en`）

**回退策略**:
```typescript
function detectLocale(): Locale {
  // 1. 用户偏好
  const stored = localStorage.getItem('jev-switch-locale');
  if (stored && isValidLocale(stored)) {
    return stored as Locale;
  }

  // 2. 浏览器语言（取前缀）
  const browserLang = navigator.language.split('-')[0];
  if (isValidLocale(browserLang)) {
    return browserLang as Locale;
  }

  // 3. 默认英文
  return 'en';
}
```

---

## §4 实施步骤（未来扩展）

### 4.1 添加新语言

**步骤**:
1. 创建新词典文件：`ui/src/i18n/ja.ts`
2. 复制 `zh.ts` 的类型定义：
   ```typescript
   import type { en } from './en';
   export const ja: Record<keyof typeof en, string> = {
     'common.reload': 'リロード',
     // ... 翻译所有键
   };
   ```
3. 更新 `Locale` 类型：
   ```typescript
   type Locale = 'en' | 'zh' | 'ja';
   ```
4. 更新 `useI18n` hook：
   ```typescript
   const dictionaries = { en, zh, ja };
   ```
5. 运行测试：
   ```bash
   npm test -- i18n.test.ts
   ```

### 4.2 翻译工作流

**建议工具**:
- **人工翻译**：关键 UI 文案（P0）
- **机器翻译 + 人工校对**：长文本、帮助文档（P1）
- **众包翻译**：社区贡献（P2）

**翻译文件格式**:
```json
{
  "common.reload": {
    "en": "Reload",
    "zh": "重载",
    "ja": "リロード"
  }
}
```

**自动化脚本**（未来可选）:
```bash
# 检查缺失的翻译
npm run i18n:check

# 导出待翻译词条
npm run i18n:export > pending-translations.json

# 导入翻译结果
npm run i18n:import pending-translations.json
```

---

## §5 注意事项

### 5.1 文案长度差异

不同语言的文案长度差异较大（如中文比英文短），需要在 UI 设计时预留足够空间：

| 语言 | 平均长度 | UI 宽度建议 |
|------|---------|-----------|
| 中文（zh） | 基准 | 1.0x |
| 英文（en） | 1.5x | 1.5x |
| 德文（de） | 1.8x | 2.0x |
| 日文（ja） | 1.2x | 1.2x |

### 5.2 日期与数字格式

不同语言的日期、数字、货币格式不同，需要使用 `Intl` API：

```typescript
// 日期格式化
new Intl.DateTimeFormat(locale).format(date);

// 数字格式化
new Intl.NumberFormat(locale).format(1234.56);
```

### 5.3 复数与性别

某些语言（如俄语、阿拉伯语）有复数规则，某些语言（如法语、德语）有性别变化。

**当前方案**：英文 / 中文都不需要复杂的复数规则

**未来扩展**：使用 `Intl.PluralRules` 或 `i18next` 库

---

## §6 最佳实践

### 6.1 禁止硬编码文案

❌ **错误**:
```typescript
<button>重载</button>
```

✅ **正确**:
```typescript
<button>{t('common.reload')}</button>
```

### 6.2 使用插值而非拼接

❌ **错误**:
```typescript
`${n} 条边`
```

✅ **正确**:
```typescript
t('routing.edges', { n })  // "routing.edges": "{n} edges"
```

### 6.3 避免过度拆分

❌ **错误**:
```typescript
t('common.delete') + ' ' + t('common.confirm')
```

✅ **正确**:
```typescript
t('card.confirmDelete')  // "card.confirmDelete": "Confirm delete?"
```

---

## §7 验收清单

- [x] en.ts 和 zh.ts 键集一致
- [x] 测试套件 i18n.test.ts 通过
- [ ] 所有 UI 组件使用 `t()` 函数，无硬编码文案
- [ ] 语言切换功能正常（LocalStorage 持久化）
- [ ] 浏览器语言自动检测正常
- [ ] CDP 断言依赖的英文字面量未变动

---

**下一步**:
1. 添加更多语言（ja, ko）
2. 实现自动化翻译检查脚本
3. 集成到 CI/CD 流程

**文档作者**: Claude Opus 4.8  
**最后更新**: 2026-09-24
