import type { ExamplePayload } from '../../examples';

interface Props {
  examples: ExamplePayload[];
  onPick: (example: ExamplePayload) => void;
  activeId?: string;
}

/**
 * 4 个示例 chips — 点击加载到 input
 * 极简风 — 等宽字体 + 细 border + hover 反色
 */
export function ExampleChips({ examples, onPick, activeId }: Props) {
  return (
    <div className="flex flex-wrap items-center gap-2">
      <span className="mr-1 font-mono text-[10px] uppercase tracking-widest text-inkSubtle">
        Examples
      </span>
      {examples.map((ex) => {
        const active = ex.id === activeId;
        return (
          <button
            key={ex.id}
            type="button"
            onClick={() => onPick(ex)}
            className={
              'inline-flex items-center gap-1.5 border px-2.5 py-1 font-mono text-xs transition-colors ' +
              (active
                ? 'border-ink bg-ink text-bg'
                : 'border-border bg-panel text-ink hover:border-ink hover:bg-ink hover:text-bg')
            }
          >
            <span className="opacity-60">/</span>
            <span>{ex.label}</span>
          </button>
        );
      })}
    </div>
  );
}
