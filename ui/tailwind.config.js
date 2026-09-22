/** @type {import('tailwindcss').Config} */
export default {
  content: ['./index.html', './src/**/*.{ts,tsx}'],
  darkMode: 'class',
  theme: {
    extend: {
      colors: {
        bg: '#0a0a0a',
        panel: '#171717',
        border: '#262626',
        accent: '#10b981',
      },
    },
  },
  plugins: [],
};