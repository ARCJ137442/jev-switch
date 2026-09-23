import { useState } from 'react';

interface Props {
  onSave: (apiKey: string) => Promise<void>;
  onCancel: () => void;
}

/**
 * Replace key 内联表单（design/01 §6.1 + 契约 04 §2）：
 * 仅 password 式输入（浏览器原生掩码），无 Show 明文按钮；
 * 保存成功后表单收起，由 toast 反馈「密钥已保存，仅显示掩码」。
 */
export function KeyForm({ onSave, onCancel }: Props) {
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

  return (
    <form
      onSubmit={(e) => void submit(e)}
      className="flex flex-wrap items-center gap-2 border-t border-border bg-soft px-4 py-2.5"
    >
      <label className="flex items-center gap-2">
        <span className="font-mono text-[10px] uppercase tracking-widest text-inkMuted">
          new key
        </span>
        <input
          type="password"
          value={value}
          onChange={(e) => setValue(e.target.value)}
          placeholder="••••••••"
          autoComplete="new-password"
          spellCheck={false}
          autoFocus
          className="h-8 w-56 border border-border bg-panel px-2 font-mono text-xs text-ink placeholder:text-inkSubtle"
          aria-label="new api key（仅密文输入，不提供明文显示）"
        />
      </label>
      <button
        type="submit"
        disabled={saving || value.length === 0}
        className="h-8 border border-primaryFill bg-primaryFill px-2.5 font-mono text-xs text-white hover:bg-primaryFillHover hover:border-primaryFillHover disabled:cursor-not-allowed disabled:opacity-50"
      >
        {saving ? 'Saving…' : 'Save'}
      </button>
      <button
        type="button"
        onClick={onCancel}
        disabled={saving}
        className="h-8 border border-border bg-panel px-2.5 font-mono text-xs text-inkMuted hover:border-primaryBright hover:text-ink disabled:opacity-50"
      >
        Cancel
      </button>
      <span className="font-mono text-[10px] uppercase tracking-widest text-inkMuted">
        masked input · 无明文回显
      </span>
    </form>
  );
}
