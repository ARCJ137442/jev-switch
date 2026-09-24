import { describe, test, expect } from 'vitest';
import { en } from '../i18n/en';
import { zh } from '../i18n/zh';

/**
 * i18n 键集一致性测试
 * 确保 en.ts 和 zh.ts 的键集完全一致，避免遗漏国际化词条
 */
describe('i18n key consistency', () => {
  test('zh and en should have exactly the same keys', () => {
    const enKeys = Object.keys(en).sort();
    const zhKeys = Object.keys(zh).sort();

    // 检查键集是否完全一致
    expect(zhKeys).toEqual(enKeys);
  });

  test('no missing keys in zh', () => {
    const enKeys = Object.keys(en);
    const zhKeys = Object.keys(zh);

    const missingInZh = enKeys.filter((key) => !(key in zh));

    expect(missingInZh).toEqual([]);
  });

  test('no extra keys in zh', () => {
    const enKeys = Object.keys(en);
    const zhKeys = Object.keys(zh);

    const extraInZh = zhKeys.filter((key) => !(key in en));

    expect(extraInZh).toEqual([]);
  });

  test('no empty values in en', () => {
    const emptyKeys = Object.entries(en)
      .filter(([_, value]) => value.trim() === '')
      .map(([key]) => key);

    expect(emptyKeys).toEqual([]);
  });

  test('no empty values in zh', () => {
    const emptyKeys = Object.entries(zh)
      .filter(([_, value]) => value.trim() === '')
      .map(([key]) => key);

    expect(emptyKeys).toEqual([]);
  });

  test('all keys follow naming convention (namespace.key)', () => {
    const invalidKeys = Object.keys(en).filter((key) => !key.includes('.'));

    expect(invalidKeys).toEqual([]);
  });
});
