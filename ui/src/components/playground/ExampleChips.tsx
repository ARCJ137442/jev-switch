import type { ExamplePayload } from '../../examples';
import { useI18n } from '../../i18n';

interface Props {
  examples: ExamplePayload[];
  onPick: (example: ExamplePayload) => void;
  activeId?: string;
}

/**
 * 示例 chips（v2 设计系统）— 点击加载到 input。
 * 激活态 = accent 实底；标签走 --font-sans / --text-sm（不再 mono+uppercase+tracking）。
 */
export function ExampleChips({ examples, onPick, activeId }: Props) {
  const { t } = useI18n();
  return (
    <div className="fade-in flex flex-wrap items-center gap-2">
      <span
        className="mr-1"
        style={{ fontSize: 'var(--text-sm)', color: 'var(--text-muted)' }}
      >
        {t('pg.examples')}
      </span>
      {examples.map((ex) => {
        const active = ex.id === activeId;
        return (
          <button
            key={ex.id}
            type="button"
            onClick={() => onPick(ex)}
            aria-pressed={active}
            title={t('ex.titleQuestions', {
              desc: ex.description,
              n: Object.keys(ex.payload.questions).length,
            })}
            className="inline-flex h-8 items-center gap-1.5 px-2.5 transition-colors"
            style={{
              fontSize: 'var(--text-sm)',
              borderRadius: 'var(--radius)',
              border: `1px solid ${active ? 'var(--accent)' : 'var(--border)'}`,
              background: active ? 'var(--accent)' : 'var(--surface)',
              color: active ? '#fff' : 'var(--text)',
              fontWeight: active ? 600 : 400,
            }}
          >
            {ex.label}
          </button>
        );
      })}
    </div>
  );
}
