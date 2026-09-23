import { edgeKey, type Route } from '../../api/admin';
import { useI18n } from '../../i18n';

interface Props {
  routes: Route[];
  errorEdges: ReadonlySet<string>;
  onPatchAt: (index: number, patch: Partial<Route>) => void;
  onDeleteAt: (index: number) => void;
  onAdd: () => void;
}

/** 单元格输入：mono 仅用于 id / 数字类值（v2 字体规则） */
const cellStyle: React.CSSProperties = {
  height: '2rem',
  width: '100%',
  border: '1px solid transparent',
  borderRadius: 'var(--radius)',
  background: 'transparent',
  padding: '0 0.375rem',
  fontFamily: 'var(--font-mono)',
  fontSize: 'var(--text-sm)',
  color: 'var(--text)',
};

const thStyle: React.CSSProperties = {
  padding: '0.5rem 0.75rem',
  fontSize: 'var(--text-xs)',
  fontWeight: 600,
  color: 'var(--text-muted)',
};

/**
 * 路由表编辑（design/01 §8 a11y：连线编辑必须有表单等价路径）。
 * 全字段键盘可编辑：left / match / right / upstream_model / priority / sticky / on_error。
 * 环标红行 + 重复 left→right 行首 ⚠ 提示；增删行走同一 debounce PUT 管线。
 */
export function RouteTableForm({ routes, errorEdges, onPatchAt, onDeleteAt, onAdd }: Props) {
  const { t } = useI18n();
  const pairCount = new Map<string, number>();
  for (const r of routes) {
    const k = edgeKey(r.left, r.right);
    pairCount.set(k, (pairCount.get(k) ?? 0) + 1);
  }

  return (
    <section
      className="overflow-hidden"
      style={{
        background: 'var(--surface)',
        border: '1px solid var(--border)',
        borderRadius: 'var(--radius)',
      }}
      aria-label="routes table editor"
    >
      <header
        className="flex items-center justify-between px-4 py-2"
        style={{ borderBottom: '1px solid var(--border)' }}
      >
        {/* 保存管线说明改为表头 tooltip（渐进披露，不常驻占位） */}
        <span
          style={{ fontSize: 'var(--text-sm)', fontWeight: 600, color: 'var(--text)' }}
          title={t('rt.footer')}
        >
          {t('rt.title')}
        </span>
        <button
          type="button"
          onClick={onAdd}
          style={{
            fontSize: 'var(--text-sm)',
            background: 'var(--surface-hover)',
            border: '1px solid var(--border)',
            borderRadius: 'var(--radius)',
            color: 'var(--text)',
            padding: '0.375rem 0.75rem',
          }}
        >
          {t('common.addRow')}
        </button>
      </header>

      <div className="overflow-x-auto">
        <table className="w-full border-collapse text-left">
          <thead>
            <tr style={{ borderBottom: '1px solid var(--border)' }}>
              <th style={thStyle}>left</th>
              <th style={{ ...thStyle, padding: '0.5rem' }}>match</th>
              <th style={thStyle}>right</th>
              <th style={thStyle}>upstream_model</th>
              <th style={thStyle}>priority</th>
              <th style={{ ...thStyle, padding: '0.5rem' }}>sticky</th>
              <th style={{ ...thStyle, padding: '0.5rem' }}>on_error</th>
              <th style={{ ...thStyle, padding: '0.5rem' }} aria-label="actions" />
            </tr>
          </thead>
          <tbody>
            {routes.length === 0 && (
              <tr>
                <td
                  colSpan={8}
                  className="px-3 py-4 text-center"
                  style={{ fontSize: 'var(--text-sm)', color: 'var(--text-muted)' }}
                >
                  {t('rt.empty')}
                </td>
              </tr>
            )}
            {routes.map((r, i) => {
              const key = edgeKey(r.left, r.right);
              const isErr = errorEdges.has(key);
              const dup = (pairCount.get(key) ?? 0) > 1;
              const dupCell: React.CSSProperties = dup
                ? { ...cellStyle, borderColor: 'var(--danger)' }
                : cellStyle;
              return (
                <tr
                  key={`${key}#${i}`}
                  style={{
                    borderBottom: '1px solid var(--border)',
                    background: isErr ? 'var(--danger-bg)' : undefined,
                  }}
                >
                  <td className="px-2 py-1">
                    <div className="flex items-center gap-1">
                      {dup && (
                        <span
                          role="img"
                          aria-label={t('rt.dup')}
                          title={t('rt.dup')}
                          style={{ color: 'var(--danger)', fontSize: 'var(--text-sm)', flexShrink: 0 }}
                        >
                          ⚠
                        </span>
                      )}
                      <input
                        value={r.left}
                        onChange={(e) => onPatchAt(i, { left: e.target.value })}
                        style={dupCell}
                        aria-label={`row ${i + 1} left`}
                        spellCheck={false}
                      />
                    </div>
                  </td>
                  <td className="px-1 py-1">
                    <select
                      value={r.match}
                      onChange={(e) => onPatchAt(i, { match: e.target.value as 'exact' | 'prefix' })}
                      style={{ ...cellStyle, fontFamily: 'var(--font-sans)' }}
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
                      style={dupCell}
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
                      style={cellStyle}
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
                      className="tabular"
                      style={{ ...cellStyle, width: '5rem' }}
                      aria-label={`row ${i + 1} priority`}
                    />
                  </td>
                  <td className="px-1 py-1">
                    <select
                      value={r.sticky ?? 'none'}
                      onChange={(e) => onPatchAt(i, { sticky: e.target.value as 'none' | 'session' })}
                      style={{ ...cellStyle, fontFamily: 'var(--font-sans)' }}
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
                      style={{ ...cellStyle, fontFamily: 'var(--font-sans)' }}
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
                      style={{
                        height: '2rem',
                        padding: '0 0.375rem',
                        fontSize: 'var(--text-sm)',
                        color: 'var(--text-muted)',
                        background: 'transparent',
                      }}
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
    </section>
  );
}
