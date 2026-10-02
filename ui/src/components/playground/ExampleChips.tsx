import { useEffect, useRef } from 'react';
import type { ExamplePayload } from '../../examples';
import { useI18n, type MessageKey } from '../../i18n';

interface Props {
  examples: ExamplePayload[];
  selectedIds: readonly string[];
  onPick: (example: ExamplePayload, exclusive: boolean) => void;
}

const LONG_PRESS_MS = 500;

function ExampleChip({ example, active, onPick }: { example: ExamplePayload; active: boolean; onPick: Props['onPick'] }) {
  const { t } = useI18n();
  const holdTimer = useRef<number | null>(null);
  const suppressClick = useRef(false);
  const clearHold = () => {
    if (holdTimer.current !== null) window.clearTimeout(holdTimer.current);
    holdTimer.current = null;
  };
  useEffect(() => clearHold, []);

  return <button
    type="button"
    onPointerDown={(event) => {
      suppressClick.current = false;
      if (event.pointerType !== 'touch' && event.pointerType !== 'pen') return;
      clearHold();
      holdTimer.current = window.setTimeout(() => {
        suppressClick.current = true;
        onPick(example, true);
        holdTimer.current = null;
      }, LONG_PRESS_MS);
    }}
    onPointerUp={clearHold}
    onPointerCancel={clearHold}
    onPointerLeave={clearHold}
    onClick={() => {
      if (suppressClick.current) { suppressClick.current = false; return; }
      onPick(example, false);
    }}
    onContextMenu={(event) => { event.preventDefault(); clearHold(); suppressClick.current = true; onPick(example, true); }}
    onKeyDown={(event) => {
      if (event.key === 'ContextMenu' || (event.shiftKey && event.key === 'F10')) {
        event.preventDefault();
        onPick(example, true);
      }
    }}
    aria-pressed={active}
    title={`${t('ex.titleQuestions', { desc: example.description, n: Object.keys(example.payload.questions).length })} · ${t('pg.shiftToToggle' as MessageKey)}`}
    className="inline-flex h-8 items-center gap-1.5 px-2.5 transition-colors"
    style={{
      fontSize: 'var(--text-sm)',
      borderRadius: 'var(--radius)',
      border: `1px solid ${active ? 'var(--accent)' : 'var(--border)'}`,
      background: active ? 'var(--accent)' : 'var(--surface)',
      color: active ? '#fff' : 'var(--text)',
      fontWeight: active ? 600 : 400,
    }}
  >{example.label}</button>;
}

/**
 * 示例 chips（v2 设计系统）— 点击加载到 input。
 * 激活态 = accent 实底；标签走 --font-sans / --text-sm（不再 mono+uppercase+tracking）。
 */
export function ExampleChips({ examples, selectedIds, onPick }: Props) {
  const { t } = useI18n();
  return (
    <div className="playground-examples min-w-0">
      <div className="playground-example-chips fade-in flex flex-wrap items-center gap-2">
      <span
        className="mr-1"
        style={{ fontSize: 'var(--text-sm)', color: 'var(--text-muted)' }}
      >
        {t('pg.examples')}
      </span>
      {examples.map((ex) => <ExampleChip key={ex.id} example={ex} active={selectedIds.includes(ex.id)} onPick={onPick} />)}
      <span className="ml-1 text-xs" style={{ color: 'var(--text-muted)' }}>
        {t('pg.selectedSamples' as MessageKey, { count: selectedIds.length })}
      </span>
      </div>
    </div>
  );
}
