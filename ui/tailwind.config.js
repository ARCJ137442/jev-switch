/** @type {import('tailwindcss').Config} */
export default {
  content: ['./index.html', './src/**/*.{ts,tsx}'],
  theme: {
    extend: {
      colors: {
        // jevplayground.com inspired — 极浅灰背景 + 深黑文字
        bg: '#fafafa',
        panel: '#ffffff',
        border: '#e5e5e5',
        borderStrong: '#0a0a0a',
        ink: '#0a0a0a',
        inkMuted: '#525252',
        inkSubtle: '#a3a3a3',
        accent: '#000000',
      },
      fontFamily: {
        sans: ['Inter', '-apple-system', 'BlinkMacSystemFont', 'Segoe UI', 'sans-serif'],
        mono: ['"JetBrains Mono"', '"Geist Mono"', '"IBM Plex Mono"', 'ui-monospace', 'SFMono-Regular', 'Menlo', 'monospace'],
      },
      borderRadius: {
        // 极简风 — 几乎不用圆角
        DEFAULT: '2px',
      },
      letterSpacing: {
        tightest: '-0.04em',
      },
    },
  },
  plugins: [],
};
