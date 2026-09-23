import { useState } from 'react';
import { useI18n } from '../../i18n';

interface Props {
  onSave: (apiKey: string) => Promise<void>;
  onCancel: () => void;
}

/**
 * Replace key 内联表单（v2 设计系统 + 契约 04 §2）：
 * 仅 password 式输入（浏览器原生掩码），无 Show 明文按钮；
 * 「只存掩码」的解释收进 input 的 title tooltip，不常驻界面（渐进披露）。
 */
export function KeyForm({ onSave, onCancel }: Props) {
  const { t } = useI18n();
  const [value, setValue] = useState('');
  const [saving, setSaving] = useState(false);

  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (value.length === 0 || saving) return;
    setSaving(true);
    try {
      await onSave(value);
      setValue('');
      onCancel(); // 收起表单
    } catch {
      // toast 已由页面反馈；保持表单打开允许重试
    } finally {
      setSaving(false);
    }
  };

  const btn: React.CSSProperties = {
    fontSize: 'var(--text-sm)',
    background: 'var(--surface)',
    border: '1px solid var(--border)',
    borderRadius: 'var(--radius)',
    color: 'var(--text)',
    padding: '0.35rem 0.7rem',
  };

  return (
    <form
      onSubmit={(e) => void submit(e)}
      className="flex flex-wrap items-center gap-2 px-4 py-3"
      style={{ borderTop: '1px solid var(--border)', background: 'var(--surface-hover)' }}
    >
      <label className="flex items-center gap-2" style={{ fontSize: 'var(--text-sm)' }}>
        <span style={{ color: 'var(--text-muted)' }}>{t('key.newKey')}</span>
        <input
          type="password"
          value={value}
          onChange={(e) => setValue(e.target.value)}
          placeholder="••••••••"
          autoComplete="new-password"
          spellCheck={false}
          autoFocus
          title={t('key.maskedHint')}
          className="h-8 w-56 px-2"
          style={{
            fontFamily: 'var(--font-mono)',
            fontSize: 'var(--text-sm)',
            background: 'var(--surface)',
            border: '1px solid var(--border)',
            borderRadius: 'var(--radius)',
            color: 'var(--text)',
          }}
          aria-label="new api key (masked input only, no plaintext reveal)"
        />
      </label>
      <button
        type="submit"
        disabled={saving || value.length === 0}
        className="disabled:cursor-not-allowed disabled:opacity-50"
        style={{
          ...btn,
          background: 'var(--accent)',
          borderColor: 'var(--accent)',
          color: '#fff',
          fontWeight: 600,
        }}
      >
        {saving ? t('common.saving') : t('common.save')}
      </button>
      <button
        type="button"
        onClick={onCancel}
        disabled={saving}
        className="disabled:opacity-50"
        style={btn}
      >
        {t('common.cancel')}
      </button>
    </form>
  );
}
