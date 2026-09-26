# 国际化 (i18n) 设计方案

**日期**: 2026-09-24
**版本**: 1.1
**状态**: 语言注册与选择机制已实施；新增语言翻译待提供
**原方案作者**: Claude Opus 4.8
**实施记录**: GPT-6 Luna

> **2026-09-24 最新方向：**用户已确认使用可扩展的多语言选择列表，后续可以添加日语、俄语、法语等。现阶段补全中文/英文体验与语言扩展机制，实施时参考 CC Switch 的语言选择、词条组织与回退方式；不能继续把语言 UI 固化为中英切换按钮，也不能把未翻译的语言显示为已可用。见 [入口网关认知对齐与实施计划](ENDPOINT-GATEWAY-ALIGNMENT-PLAN.md)。本文历史测试勾选不代表当前测试存在或已经通过，后续按实际文件与运行结果验收。

---

## §1 当前实现

### 1.1 架构

**词典与注册**:
- `ui/src/i18n/en.ts` — 英文词典（默认语言，CDP 断言依赖）
- `ui/src/i18n/zh.ts` — 中文词典（类型强制与 en 键集一致）
- `ui/src/i18n/languages.ts` — 可用语言注册表、语言代码校验、浏览器语言匹配与选择列表数据
- `ui/src/i18n/core.ts` — 持久化选择、英文缺键回退、`<html lang>` 同步
- `ui/src/components/ui/LangToggle.tsx` — 从注册表渲染的语言下拉选择器；目前只列出已完整提供的 English 与简体中文

`languageRegistry` 是唯一可用语言目录。新增语言前，先补齐与英文键集一致的词典，再向注册表添加代码、显示名称、HTML语言标记和词典；选择器、校验、浏览器匹配及翻译表会读取该目录。不能仅为展示语言选项而注册未完成翻译的语言。

初始化顺序：可用的 `localStorage['jev_lang']` → 注册表中完全匹配的浏览器 locale → 注册表中匹配主语言代码的 locale → 英文默认值。无效存储值回退英文。手动选择立即写入 `jev_lang` 并更新 `<html lang>`；存储不可用时继续在当前会话切换。中文页面写 `zh-CN`，英文页面写 `en`。

当前翻译函数对目标语言缺失键回退英文，再回退键名。新增语言应通过词典键集检查确保覆盖，不将回退机制视为翻译完成。

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

## §2 测试保障与本轮验证

### 2.1 i18n 键集一致性测试

历史设计稿曾记录 `ui/src/tests/i18n.test.ts`，当前仓库没有该测试文件，不能将旧勾选视为本轮测试结果。项目目前没有 i18n 专用测试命令。

本轮变更后应运行 `cd ui && npm run lint`（等价于 TypeScript 全项目检查），并在浏览器验收：存储为 `zh` 时显示中文并设 `html.lang=zh-CN`；无效存储值时回退英文；无存储时 `zh-*` 浏览器偏好选择中文、其他偏好选英文；切换后两处语言选择器同步并持久化。浏览器行为尚未在此实施记录中声称通过。

---

## §3 多语言扩展设计

### 3.0 参考实现与取舍

已核对 CC Switch 官方仓库 `src/i18n/index.ts`：
[官方源码](https://github.com/farion1231/cc-switch/blob/main/src/i18n/index.ts)。该实现把 locale 代码和资源集中注册，通过持久化选择优先初始化，再识别浏览器语言并回退到默认语言；其 `fallbackLng: "en"` 负责缺失词条回退。Jev Switch 采用相同的注册表驱动语言选项、偏好读取与默认回退思路。CC Switch 目前包含 `zh`、`zh-TW`、`en`、`ja`，并采用 i18next；Jev Switch 当前只注册已完整翻译的 `en` 和 `zh`，沿用现有轻量词典与 React Context，不引入新的 i18n 依赖。Jev Switch 使用既有 `jev_lang` 存储键，并将中文文档语言标记为 `zh-CN`。

### 3.1 目标语言优先级

| 优先级 | 语言 | 原因 |
|-------|------|------|
| P0 | 英文（en） | 默认语言，国际用户 |
| P0 | 中文（zh） | 主要用户群体 |
| 后续 | 日文（ja） | 翻译完成并加入注册表后开放 |
| 后续 | 韩文（ko） | 翻译完成并加入注册表后开放 |
| 后续 | 法文（fr） | 翻译完成并加入注册表后开放 |
| 后续 | 德文（de） | 翻译完成并加入注册表后开放 |

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
1. 用户手动选择（localStorage: `jev_lang`）
2. 浏览器语言（`navigator.language`，不可用时读取 `navigator.languages[0]`）
3. 默认英文（`en`）

**回退策略**:
```typescript
function detectLocale(): Locale {
  // 1. 用户偏好
  const stored = localStorage.getItem('jev_lang');
  if (stored && isRegisteredLocale(stored)) {
    return stored;
  }

  // 2. 浏览器语言（取前缀）
  const browserLang = matchRegisteredLocale(navigator.language);
  if (browserLang) {
    return browserLang;
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

## §7 实施验收记录

- [x] 语言列表只包含注册表中已支持的英文和简体中文。
- [x] 语言选择调用 `setLang`，旧 `toggleLang` API 保留并动态轮换注册语言。
- [x] 选择写入 `jev_lang`；无效存储值回退英文；无存储时匹配支持的浏览器语言，其他语言回退英文。
- [x] 初始化及切换同步 `<html lang>`（英文 `en`、简体中文 `zh-CN`）。
- [x] `cd ui && npm run lint` 通过；核心运行时检查覆盖存储优先级、无效值回退、浏览器匹配、默认语言、无效手动输入防护、持久化与 `html.lang`。
- [x] 浏览器 locale 匹配从语言注册表动态计算；可用纯函数用临时 `ja` 注册表条目验证 `ja-JP` 按主语言代码匹配，不需要也不应把未翻译日语加入产品注册表。
- [x] 浏览器端下拉切换、刷新恢复与 HTML lang 已实测；2026-09-25 继续覆盖 cloud 管理员/只读界面及英文 390px 窄屏，见实施与联调记录。
- [x] 可用列表当前只有 en/zh，未将尚未完成翻译的语言注册为可用；新增语言仍遵循完整翻译后注册的门槛。

本轮只记录确有代码与命令结果支撑的状态。没有名为 `ui/src/tests/i18n.test.ts` 的当前测试文件，历史设计中的该测试勾选不代表本轮测试结果。

**文档作者**: 原方案 Claude Opus 4.8；实施记录 GPT-6 Luna
**最后更新**: 2026-09-24
