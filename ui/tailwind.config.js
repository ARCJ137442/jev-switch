/** @type {import('tailwindcss').Config} */
export default {
  content: ['./index.html', './src/**/*.{ts,tsx}'],
  theme: {
    extend: {
      colors: {
        // design/01 §4 tokens — 数值与 src/styles/tokens.css 保持一致（改一处同步另一处）
        bg: '#fafafa',
        panel: '#ffffff',
        border: '#e5e5e5',
        borderStrong: '#0a0a0a',
        ink: '#0a0a0a',
        inkMuted: '#525252',
        inkSubtle: '#a3a3a3',
        accent: '#000000',
        // 语义色 — 仅表状态（ok/warn/danger）与 DAG 边
        ok: '#059669',
        warn: '#d97706',
        danger: '#dc2626',
        edge: '#0a0a0a',
        edgeIdle: '#d4d4d4',
      },
      fontFamily: {
        sans: ['Inter', '-apple-system', 'BlinkMacSystemFont', 'Segoe UI', 'sans-serif'],
        mono: ['"JetBrains Mono"', '"Geist Mono"', '"IBM Plex Mono"', 'ui-monospace', 'SFMono-Regular', 'Menlo', 'monospace'],
      },
      borderRadius: {
        // design/01 §4.3 — 圆角 0（rounded-full 仅限状态圆点）
        DEFAULT: '0',
      },
      letterSpacing: {
        tightest: '-0.04em',
      },
    },
  },
  plugins: [],
};
