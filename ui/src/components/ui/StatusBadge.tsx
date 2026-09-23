import type { ReactNode } from 'react';

export type BadgeTone = 'ok' | 'warn' | 'danger' | 'info' | 'muted';

/**
 * 状态胶囊（方案 B1：100 tint 底 + 700 文字 + 实心点/图标；rounded-full 例外）。
 * 结构学 TeamSense StatusBadge（图标 + label，不只靠色相）；
 * 配色走 B 表 —— tone 类名静态写全，Tailwind JIT 才能扫到。
 * 文字对比度均 ≥4.5:1（02 §5 实测表）。
 */
const TONE_CLS: Record<BadgeTone, string> = {
  ok: 'bg-okBg text-ok',
  warn: 'bg-warnBg text-warn',
  danger: 'bg-dangerBg text-danger',
  info: 'bg-infoBg text-info',
  muted: 'bg-soft text-inkMuted',
};

const TONE_ICON: Record<BadgeTone, ReactNode> = {
  ok: (
    <svg viewBox="0 0 24 24" width="12" height="12" fill="none" stroke="currentColor" strokeWidth="2.4" strokeLinecap="round" strokeLinejoin="round" aria-hidden>
      <circle cx="12" cy="12" r="9" />
      <path d="M8 12.5l2.5 2.5L16 9.5" />
    </svg>
  ),
  warn: (
    <svg viewBox="0 0 24 24" width="12" height="12" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round" aria-hidden>
      <path d="M12 4l9 16H3z" />
      <path d="M12 10v4M12 17v.5" />
    </svg>
  ),
  danger: (
    <svg viewBox="0 0 24 24" width="12" height="12" fill="none" stroke="currentColor" strokeWidth="2.4" strokeLinecap="round" strokeLinejoin="round" aria-hidden>
      <circle cx="12" cy="12" r="9" />
      <path d="M9 9l6 6M15 9l-6 6" />
    </svg>
  ),
  info: (
    <svg viewBox="0 0 24 24" width="12" height="12" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round" aria-hidden>
      <circle cx="12" cy="12" r="9" />
      <path d="M12 11v5M12 8v.5" />
    </svg>
  ),
  muted: (
    <span className="inline-block h-1.5 w-1.5 rounded-full bg-inkSubtle" aria-hidden />
  ),
};

interface Props {
  tone?: BadgeTone;
  children: ReactNode;
  className?: string;
  'aria-live'?: 'off' | 'polite' | 'assertive';
}

export function StatusBadge({ tone = 'muted', children, className = '', ...rest }: Props) {
  return (
    <span
      {...rest}
      className={`inline-flex items-center gap-1.5 whitespace-nowrap rounded-full px-2 py-0.5 text-[11px] font-semibold leading-none ${TONE_CLS[tone]} ${className}`.trim()}
    >
      {TONE_ICON[tone]}
      {children}
    </span>
  );
}
