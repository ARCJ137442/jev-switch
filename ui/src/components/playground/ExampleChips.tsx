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
    <div className="playground-examples min-w-0">
      <label className="playground-example-select items-center gap-2 text-sm" style={{ color: 'var(--text-muted)' }}>
        <span className="shrink-0">{t('pg.examples')}</span>
        <select aria-label={t('pg.examples')} value={activeId ?? ''}
          onChange={event => { const example = examples.find(item => item.id === event.target.value); if (example) onPick(example); }}
          className="h-9 min-w-0 flex-1 border px-2" style={{ background: 'var(--surface)', color: 'var(--text)' }}>
          {examples.map(example => <option key={example.id} value={example.id}>{example.label}</option>)}
        </select>
      </label>
      <div className="playground-example-chips fade-in flex flex-wrap items-center gap-2">
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
    </div>
  );
}
