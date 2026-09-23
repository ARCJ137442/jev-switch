import type { ExamplePayload } from '../../examples';
import { useI18n } from '../../i18n';

interface Props {
  examples: ExamplePayload[];
  onPick: (example: ExamplePayload) => void;
  activeId?: string;
}

/**
 * 示例 chips — 点击加载到 input。
 * 激活/悬停 = 主蓝实底（CC Switch 手法），替代原黑白反色。
 */
export function ExampleChips({ examples, onPick, activeId }: Props) {
  const { t } = useI18n();
  return (
    <div className="flex flex-wrap items-center gap-2">
      <span className="mr-1 font-mono text-[10px] uppercase tracking-widest text-inkMuted">
        {t('pg.examples')}
      </span>
      {examples.map((ex) => {
        const active = ex.id === activeId;
        return (
          <button
            key={ex.id}
            type="button"
            onClick={() => onPick(ex)}
            title={t('ex.titleQuestions', { desc: ex.description, n: Object.keys(ex.payload.questions).length })}
            className={
              'inline-flex h-8 items-center gap-1.5 border px-2.5 font-mono text-xs transition-colors ' +
              (active
                ? 'border-primaryFill bg-primaryFill text-white'
                : 'border-border bg-panel text-ink hover:border-primaryBright hover:bg-soft hover:text-primary')
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
