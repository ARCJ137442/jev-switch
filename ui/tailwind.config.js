/** @type {import('tailwindcss').Config} */
export default {
  content: ['./index.html', './src/**/*.{ts,tsx}'],
  theme: {
    extend: {
      colors: {
        // 方案 B 色板（docs/design/02 §3.2）— 全部走 CSS 变量，浅/深主题同名换值
        // 数值与 src/styles/tokens.css 保持一致（改一处同步另一处）
        bg: 'var(--bg)',
        panel: 'var(--panel)',
        soft: 'var(--soft)',
        border: 'var(--border)',
        borderStrong: 'var(--primary-bright)',
        ink: 'var(--ink)',
        inkMuted: 'var(--ink-muted)',
        inkSubtle: 'var(--ink-subtle)',
        accent: 'var(--primary)',
        // 交互蓝（B3/B4）
        primary: 'var(--primary)',
        primaryFill: 'var(--primary-fill)',
        primaryFillHover: 'var(--primary-fill-hover)',
        primaryBright: 'var(--primary-bright)',
        overlay: 'var(--overlay)',
        // 语义状态：文字(700) / bg(pill 100) / dot(500-600)
        ok: 'var(--ok)',
        okBg: 'var(--ok-bg)',
        okDot: 'var(--ok-dot)',
        warn: 'var(--warn)',
        warnBg: 'var(--warn-bg)',
        warnDot: 'var(--warn-dot)',
        danger: 'var(--danger)',
        dangerBg: 'var(--danger-bg)',
        dangerFill: 'var(--danger-fill)',
        info: 'var(--info)',
        infoBg: 'var(--info-bg)',
        // DAG 边
        edge: 'var(--edge)',
        edgeIdle: 'var(--edge-idle)',
      },
      fontFamily: {
        sans: ['Inter', '-apple-system', 'BlinkMacSystemFont', 'Segoe UI', 'sans-serif'],
        mono: ['"JetBrains Mono"', '"Geist Mono"', '"IBM Plex Mono"', 'ui-monospace', 'SFMono-Regular', 'Menlo', 'monospace'],
      },
      borderRadius: {
        // TeamSense：控件 6px / 面板 7px（rounded-full 仅状态胶囊与圆点）
        DEFAULT: '6px',
        card: '7px',
        ctl: '6px',
      },
      letterSpacing: {
        tightest: '-0.04em',
      },
    },
  },
  plugins: [],
};
