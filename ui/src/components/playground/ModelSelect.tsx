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
 * Model 选择器 — 等宽字体 + 极简标签
 * 不再用大段说明文字，只显示 model id 和 upstream hint
 */
export function ModelSelect({ options, value, onChange, serverReachable }: Props) {
  const current = options.find((o) => o.value === value);
  return (
    <section className="border border-border bg-panel">
      <header className="flex items-baseline justify-between border-b border-border px-4 py-2.5">
        <h2 className="font-mono text-[10px] font-semibold uppercase tracking-widest text-inkSubtle">
          Model
        </h2>
        <span className="font-mono text-[10px] uppercase tracking-widest text-inkSubtle">
          {serverReachable ? 'from server' : 'fallback'}
        </span>
      </header>
      <div className="px-4 py-3">
        <label className="flex items-center gap-3">
          <span className="font-mono text-xs text-inkSubtle">/v1/systemone</span>
          <select
            value={value}
            onChange={(e) => onChange(e.target.value)}
            className="h-8 flex-1 appearance-none border border-border bg-bg px-3 font-mono text-[13px] text-ink tabular"
            aria-label="select model"
          >
            {options.map((opt) => (
              <option key={opt.value} value={opt.value}>
                {opt.value}
              </option>
            ))}
          </select>
          {current?.upstream && (
            <span className="font-mono text-[10px] uppercase tracking-widest text-inkSubtle">
              → {current.upstream}
            </span>
          )}
        </label>
      </div>
    </section>
  );
}
