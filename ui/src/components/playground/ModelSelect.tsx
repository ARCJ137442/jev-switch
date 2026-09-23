interface Option {
  value: string;
  label: string;
  upstream?: string;
}

interface Props {
  options: Option[];
  value: string;
  onChange: (v: string) => void;
  serverReachable: boolean;
}

/**
 * Model 选择器（v2 设计系统）— 人话 label「Model」+ model id 等宽显示。
 * 审查报告 UI-AUDIT-2026-09-23：删掉常驻的 `/v1/systemone` 技术路径标签（渐进披露），
 * 删掉 mono+uppercase+tracking-widest 终端腔，select 字号统一 --text-sm。
 */
export function ModelSelect({ options, value, onChange, serverReachable }: Props) {
  const current = options.find((o) => o.value === value);
  return (
    <section
      className="fade-in overflow-hidden"
      style={{
        background: 'var(--surface)',
        border: '1px solid var(--border)',
        borderRadius: 'var(--radius)',
      }}
    >
      <header
        className="flex items-baseline justify-between px-4 py-2.5"
        style={{ borderBottom: '1px solid var(--border)' }}
      >
        <h2 className="font-semibold" style={{ fontSize: 'var(--text-sm)' }}>
          Model
        </h2>
        <span style={{ fontSize: 'var(--text-xs)', color: 'var(--text-subtle)' }}>
          {serverReachable ? 'from server' : 'fallback'}
        </span>
      </header>
      <div className="flex items-center gap-3 px-4 py-3">
        <select
          value={value}
          onChange={(e) => onChange(e.target.value)}
          className="tabular h-8 min-w-0 flex-1 appearance-none px-3"
          style={{
            border: '1px solid var(--border)',
            borderRadius: 'var(--radius)',
            background: 'var(--surface-hover)',
            color: 'var(--text)',
            fontFamily: 'var(--font-mono)',
            fontSize: 'var(--text-sm)',
          }}
          aria-label="select model"
        >
          {options.map((opt) => (
            <option key={opt.value} value={opt.value}>
              {opt.value}
            </option>
          ))}
        </select>
        {current?.upstream && (
          <span
            className="shrink-0"
            style={{ fontSize: 'var(--text-xs)', color: 'var(--text-muted)' }}
          >
            → <span style={{ fontFamily: 'var(--font-mono)' }}>{current.upstream}</span>
          </span>
        )}
      </div>
    </section>
  );
}
