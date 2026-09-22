import type { ProviderInfo } from '../api';

interface Props {
  providers: ProviderInfo[];
  onToggle: (id: ProviderInfo['id'], enabled: boolean) => void;
}

export function ProvidersPanel({ providers, onToggle }: Props) {
  return (
    <section className="rounded-lg border border-border bg-panel p-4">
      <header className="mb-3 flex items-center justify-between">
        <h2 className="text-sm font-semibold uppercase tracking-wider text-neutral-400">
          Providers
        </h2>
        <span className="text-xs text-neutral-500">{providers.length} configured</span>
      </header>
      <ul className="space-y-2">
        {providers.map((p) => (
          <li
            key={p.id}
            className="flex items-start justify-between gap-3 rounded border border-border bg-black/30 p-3"
          >
            <div className="min-w-0 flex-1">
              <div className="flex items-center gap-2">
                <span
                  className={
                    'inline-block h-2 w-2 rounded-full ' +
                    (p.enabled ? 'bg-accent' : 'bg-neutral-600')
                  }
                />
                <span className="font-medium text-neutral-100">{p.label}</span>
                <span className="font-mono text-xs text-neutral-500">({p.id})</span>
              </div>
              <p className="mt-1 truncate text-xs text-neutral-500" title={p.base}>
                {p.description}
              </p>
              <p className="truncate font-mono text-[11px] text-neutral-600" title={p.base}>
                {p.base}
              </p>
            </div>
            <label className="flex shrink-0 cursor-pointer items-center gap-2">
              <span className="text-xs text-neutral-400">
                {p.enabled ? 'enabled' : 'disabled'}
              </span>
              <span
                className={
                  'relative inline-block h-5 w-9 rounded-full transition-colors ' +
                  (p.enabled ? 'bg-accent' : 'bg-neutral-700')
                }
              >
                <span
                  className={
                    'absolute top-0.5 h-4 w-4 rounded-full bg-white transition-all ' +
                    (p.enabled ? 'left-4' : 'left-0.5')
                  }
                />
              </span>
              <input
                type="checkbox"
                className="sr-only"
                checked={p.enabled}
                onChange={(e) => onToggle(p.id, e.target.checked)}
                aria-label={`toggle ${p.label}`}
              />
            </label>
          </li>
        ))}
      </ul>
    </section>
  );
}