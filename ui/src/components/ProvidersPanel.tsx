import type { ProviderInfo } from '../api';

interface Props {
  providers: ProviderInfo[];
  onToggle: (id: ProviderInfo['id'], enabled: boolean) => void;
}

/**
 * Providers 面板 — 横向条状列表，无圆角阴影
 * 极简风 — 左侧 status dot + 名称 + base URL；右侧 toggle
 */
export function ProvidersPanel({ providers, onToggle }: Props) {
  return (
    <section className="border border-border bg-panel">
      <header className="flex items-baseline justify-between border-b border-border px-4 py-2.5">
        <h2 className="font-mono text-[10px] font-semibold uppercase tracking-widest text-inkSubtle">
          Providers
        </h2>
        <span className="font-mono text-xs text-inkSubtle tabular">
          {providers.filter((p) => p.enabled).length}/{providers.length} active
        </span>
      </header>

      <ul className="divide-y divide-border">
        {providers.map((p) => (
          <li key={p.id} className="flex items-center justify-between gap-4 px-4 py-3">
            <div className="min-w-0 flex-1">
              <div className="flex items-center gap-2.5">
                <span
                  className={
                    'inline-block h-1.5 w-1.5 rounded-full ' +
                    (p.enabled ? 'bg-emerald-500' : 'bg-inkSubtle')
                  }
                />
                <span className="text-sm font-semibold text-ink">{p.label}</span>
                <span className="font-mono text-xs text-inkSubtle">{p.id}</span>
              </div>
              <p className="mt-0.5 truncate text-xs text-inkMuted" title={p.description}>
                {p.description}
              </p>
              <p className="mt-0.5 truncate font-mono text-[11px] text-inkSubtle" title={p.base}>
                {p.base}
              </p>
            </div>

            <label className="flex shrink-0 cursor-pointer items-center gap-2">
              <span className="font-mono text-[10px] uppercase tracking-widest text-inkSubtle">
                {p.enabled ? 'on' : 'off'}
              </span>
              <span
                role="switch"
                aria-checked={p.enabled}
                onClick={() => onToggle(p.id, !p.enabled)}
                onKeyDown={(e) => {
                  if (e.key === ' ' || e.key === 'Enter') {
                    e.preventDefault();
                    onToggle(p.id, !p.enabled);
                  }
                }}
                tabIndex={0}
                className={
                  'relative inline-block h-5 w-9 border transition-colors ' +
                  (p.enabled ? 'border-ink bg-ink' : 'border-inkSubtle bg-bg')
                }
              >
                <span
                  className={
                    'absolute top-0.5 h-3 w-3 transition-all ' +
                    (p.enabled ? 'left-[18px] bg-bg' : 'left-0.5 bg-ink')
                  }
                />
              </span>
            </label>
          </li>
        ))}
      </ul>
    </section>
  );
}
