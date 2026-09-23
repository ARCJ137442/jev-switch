import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

// https://vitejs.dev/config/
export default defineConfig({
  plugins: [react()],
  server: {
    port: 5173,
    // daemon 的 CORS 白名单冻结为 127.0.0.1:5173 / localhost:5173
    // （rs/crates/jev-switch-daemon/src/lib.rs · cors_allowlist_origins_frozen 测试）。
    // 端口一漂移，所有 admin 请求就被跨源拦掉，UI 表现为「daemon unreachable」
    // 却无任何 JS 报错 —— 极难排查。故必须 strictPort：宁可起不来，不要静默漂移。
    strictPort: true,
    host: '127.0.0.1',
  },
  build: {
    outDir: 'dist',
    sourcemap: false,
  },
});