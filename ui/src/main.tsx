import { Component, StrictMode, type ErrorInfo, type ReactNode } from 'react';
import { createRoot } from 'react-dom/client';
import App from './App';
import './styles/tokens.css';
import './index.css';

/* 主题初始值（块 2）：localStorage['jev_theme'] 优先，否则遵 prefers-color-scheme。
   在首帧渲染前写 data-theme，避免浅→深闪烁。 */
function initTheme(): void {
  let stored: string | null = null;
  try {
    stored = localStorage.getItem('jev_theme');
  } catch {
    /* 隐私模式等 → 走系统偏好 */
  }
  const theme =
    stored === 'dark' || stored === 'light'
      ? stored
      : window.matchMedia?.('(prefers-color-scheme: dark)').matches
        ? 'dark'
        : 'light';
  document.documentElement.dataset.theme = theme;
}
initTheme();

const rootEl = document.getElementById('root');
if (!rootEl) throw new Error('#root not found');

interface BoundaryProps {
  children: ReactNode;
}
interface BoundaryState {
  error: Error | null;
}

/**
 * Top-level ErrorBoundary — surfaces render-time errors in the DOM instead of
 * silently leaving `<div id="root">` empty. Without this, a downstream render
 * error makes the entire React tree disappear with no on-screen feedback,
 * which is exactly the symptom this app exhibited before this commit.
 */
class RootErrorBoundary extends Component<BoundaryProps, BoundaryState> {
  state: BoundaryState = { error: null };

  static getDerivedStateFromError(error: Error): BoundaryState {
    return { error };
  }

  override componentDidCatch(error: Error, info: ErrorInfo): void {
    // eslint-disable-next-line no-console
    console.error('[RootErrorBoundary]', error, info);
  }

  override render(): ReactNode {
    if (this.state.error) {
      const msg = this.state.error?.message ?? String(this.state.error);
      return (
        <div
          style={{
            padding: 24,
            fontFamily: 'ui-monospace, SFMono-Regular, Menlo, monospace',
            // design/01 §4 tokens — 崩溃兜底屏也走语义色，不引入屏外色值
            color: 'var(--danger)',
            background: 'var(--panel)',
            border: '1px solid var(--danger)',
            margin: 24,
          }}
        >
          <div style={{ fontWeight: 700, marginBottom: 8 }}>
            Jev-Switch UI crashed
          </div>
          <pre style={{ whiteSpace: 'pre-wrap', margin: 0 }}>{msg}</pre>
        </div>
      );
    }
    return this.props.children;
  }
}

createRoot(rootEl).render(
  <StrictMode>
    <RootErrorBoundary>
      <App />
    </RootErrorBoundary>
  </StrictMode>,
);
