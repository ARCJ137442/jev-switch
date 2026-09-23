import { edgeKey, type Route } from '../../api/admin';
import { useI18n } from '../../i18n';

interface Props {
  routes: Route[];
  errorEdges: ReadonlySet<string>;
  onPatchAt: (index: number, patch: Partial<Route>) => void;
  onDeleteAt: (index: number) => void;
  onAdd: () => void;
}

const cell =
  'h-8 w-full border border-transparent bg-transparent px-1.5 font-mono text-xs text-ink tabular focus:border-primaryBright focus:bg-soft';

/**
 * 路由表编辑（design/01 §8 a11y：连线编辑必须有表单等价路径）。
 * 全字段键盘可编辑：left / match / right / upstream_model / priority / sticky / on_error。
 * 环标红行（dangerBg tint）+ 重复 left→right 提示；增删行走同一 debounce PUT 管线。
 */
export function RouteTableForm({ routes, errorEdges, onPatchAt, onDeleteAt, onAdd }: Props) {
  const { t } = useI18n();
  const pairCount = new Map<string, number>();
  for (const r of routes) {
    const k = edgeKey(r.left, r.right);
    pairCount.set(k, (pairCount.get(k) ?? 0) + 1);
  }

  return (
    <section className="overflow-hidden rounded-card border border-border bg-panel" aria-label="routes table editor">
      <header className="flex items-center justify-between border-b border-border px-4 py-2">
        <span className="font-mono text-[10px] font-semibold uppercase tracking-widest text-inkMuted">
          {t('rt.title')}
        </span>
        <button
          type="button"
          onClick={onAdd}
          className="h-8 border border-border bg-panel px-2.5 font-mono text-xs text-ink hover:border-primaryBright hover:bg-soft"
        >
          {t('common.addRow')}
        </button>
      </header>

      <div className="overflow-x-auto">
        <table className="w-full border-collapse text-left">
          <thead>
            <tr className="border-b border-border font-mono text-[10px] uppercase tracking-widest text-inkMuted">
              <th className="px-3 py-2 font-semibold">left</th>
              <th className="px-2 py-2 font-semibold">match</th>
              <th className="px-3 py-2 font-semibold">right</th>
              <th className="px-3 py-2 font-semibold">upstream_model</th>
              <th className="px-3 py-2 font-semibold">priority</th>
              <th className="px-2 py-2 font-semibold">sticky</th>
              <th className="px-2 py-2 font-semibold">on_error</th>
              <th className="px-2 py-2" aria-label="actions" />
            </tr>
          </thead>
          <tbody>
            {routes.length === 0 && (
              <tr>
                <td colSpan={8} className="px-3 py-4 text-center text-sm text-inkMuted">
                  {t('rt.empty')}
                </td>
              </tr>
            )}
            {routes.map((r, i) => {
              const key = edgeKey(r.left, r.right);
              const isErr = errorEdges.has(key);
              const dup = (pairCount.get(key) ?? 0) > 1;
              return (
                <tr
                  key={`${key}#${i}`}
                  className={
                    'border-b border-border ' + (isErr ? 'bg-dangerBg' : 'hover:bg-soft')
                  }
                >
                  <td className="px-2 py-1">
                    <input
                      value={r.left}
                      onChange={(e) => onPatchAt(i, { left: e.target.value })}
                      className={cell + (dup ? ' border-danger' : '')}
                      aria-label={`row ${i + 1} left`}
                      spellCheck={false}
                    />
                  </td>
                  <td className="px-1 py-1">
                    <select
                      value={r.match}
                      onChange={(e) => onPatchAt(i, { match: e.target.value as 'exact' | 'prefix' })}
                      className={cell}
                      aria-label={`row ${i + 1} match`}
                    >
                      <option value="exact">exact</option>
                      <option value="prefix">prefix</option>
                    </select>
                  </td>
                  <td className="px-2 py-1">
                    <input
                      value={r.right}
                      onChange={(e) => onPatchAt(i, { right: e.target.value })}
                      className={cell + (dup ? ' border-danger' : '')}
                      aria-label={`row ${i + 1} right`}
                      spellCheck={false}
                    />
                  </td>
                  <td className="px-2 py-1">
                    <input
                      value={r.upstream_model ?? ''}
                      onChange={(e) =>
                        onPatchAt(
                          i,
                          e.target.value.length > 0
                            ? { upstream_model: e.target.value }
                            : { upstream_model: undefined },
                        )
                      }
                      placeholder="—"
                      className={cell + ' placeholder:text-inkSubtle'}
                      aria-label={`row ${i + 1} upstream model`}
                      spellCheck={false}
                    />
                  </td>
                  <td className="px-2 py-1">
                    <input
                      type="number"
                      value={r.priority}
                      onChange={(e) => {
                        const n = Number(e.target.value);
                        if (Number.isFinite(n)) onPatchAt(i, { priority: Math.trunc(n) });
                      }}
                      className={cell + ' w-20 tabular'}
                      aria-label={`row ${i + 1} priority`}
                    />
                  </td>
                  <td className="px-1 py-1">
                    <select
                      value={r.sticky ?? 'none'}
                      onChange={(e) => onPatchAt(i, { sticky: e.target.value as 'none' | 'session' })}
                      className={cell}
                      aria-label={`row ${i + 1} sticky`}
                    >
                      <option value="none">none</option>
                      <option value="session">session</option>
                    </select>
                  </td>
                  <td className="px-1 py-1">
                    <select
                      value={r.on_error ?? 'next'}
                      onChange={(e) => onPatchAt(i, { on_error: e.target.value as 'next' | 'fail' })}
                      className={cell}
                      aria-label={`row ${i + 1} on error`}
                    >
                      <option value="next">next</option>
                      <option value="fail">fail</option>
                    </select>
                  </td>
                  <td className="px-2 py-1 text-right">
                    <button
                      type="button"
                      onClick={() => onDeleteAt(i)}
                      aria-label={`delete row ${i + 1}`}
                      className="h-8 px-1.5 font-mono text-xs text-inkMuted hover:text-danger"
                    >
                      {t('rt.del')}
                    </button>
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>
      <footer className="border-t border-border px-4 py-2 font-mono text-[10px] uppercase tracking-widest text-inkMuted">
        {t('rt.footer')}
      </footer>
    </section>
  );
}
