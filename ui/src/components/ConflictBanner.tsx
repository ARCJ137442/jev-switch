interface Props {
  onReload: () => void;
  onOverwrite: () => void;
  busy?: boolean;
}

/**
 * 冲突横幅（design/01 §5 / §7，契约 04 §3）：
 * 「配置已在外部修改」→ 重载（默认，主 CTA 蓝）/ 覆盖；warn 描边，禁止静默丢弃手改。
 */
export function ConflictBanner({ onReload, onOverwrite, busy = false }: Props) {
  return (
    <div
      role="alert"
      className="border-b border-warn bg-warnBg"
      data-testid="conflict-banner"
    >
      <div className="mx-auto flex max-w-7xl flex-wrap items-center justify-between gap-3 px-6 py-2.5">
        <span className="flex items-center gap-2 text-sm text-ink">
          <span className="inline-block h-1.5 w-1.5 rounded-full bg-warnDot" aria-hidden />
          配置已在外部修改
        </span>
        <span className="flex items-center gap-2">
          <button
            type="button"
            onClick={onReload}
            disabled={busy}
            className="h-8 border border-primaryFill bg-primaryFill px-3 font-mono text-xs text-white hover:bg-primaryFillHover hover:border-primaryFillHover disabled:opacity-50"
          >
            重载
          </button>
          <button
            type="button"
            onClick={onOverwrite}
            disabled={busy}
            className="h-8 border border-border bg-panel px-3 font-mono text-xs text-ink hover:border-primaryBright hover:bg-soft disabled:opacity-50"
          >
            覆盖
          </button>
        </span>
      </div>
    </div>
  );
}
