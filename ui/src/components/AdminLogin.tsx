import { useEffect, useRef, useState } from 'react';
import { loginAdmin } from '../api/admin';

/**
 * #43 cloud 态 admin 登录小窗（任务书 B：**极简能用**，视觉打磨归 UI 线 #42）。
 *
 * 触发：Providers / Routing 页任意 admin 请求 401/403（会话缺失/过期）——
 * api/admin.ts 全局回调 → Shell 渲染本组件。
 * 流程：输密码 → `POST /v1/admin/login` → 服务端发短时会话 token（存
 * localStorage，密码**不落盘**）→ `onSuccess`（Shell：收窗 + 刷新页面重拉数据）。
 * local 态：服务端不产生 401 → 本窗永不出现（零打扰）。
 */
interface AdminLoginProps {
  onSuccess: () => void;
}

export function AdminLogin({ onSuccess }: AdminLoginProps) {
  const [password, setPassword] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (busy) return;
    setBusy(true);
    setError(null);
    try {
      await loginAdmin(password);
      setPassword(''); // 密码只内存瞬存，成功即清
      onSuccess();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-overlay"
      role="dialog"
      aria-modal="true"
      aria-labelledby="admin-login-title"
    >
      <form
        onSubmit={(e) => void submit(e)}
        className="w-full max-w-sm rounded-card border border-border bg-panel p-6"
      >
        <h2
          id="admin-login-title"
          className="font-mono text-[10px] font-semibold uppercase tracking-widest text-inkMuted"
        >
          Admin login
        </h2>
        <p className="mt-2 font-mono text-xs text-inkMuted">
          cloud 态管理需登录（会话缺失或已过期）
        </p>
        <input
          ref={inputRef}
          type="password"
          value={password}
          onChange={(e) => setPassword(e.target.value)}
          placeholder="admin password"
          autoComplete="current-password"
          className="mt-4 h-9 w-full border border-border bg-soft px-3 font-mono text-xs text-ink placeholder:text-inkSubtle focus:border-primaryBright focus:outline-none"
        />
        {error !== null && (
          <p className="mt-2 font-mono text-xs text-danger" role="alert">
            {error}
          </p>
        )}
        <div className="mt-4 flex justify-end gap-2">
          <button
            type="submit"
            disabled={busy || password.length === 0}
            className="h-8 border border-primaryFill bg-primaryFill px-4 font-mono text-xs text-white hover:bg-primaryFillHover hover:border-primaryFillHover disabled:cursor-not-allowed disabled:opacity-50"
          >
            {busy ? '…' : '登录'}
          </button>
        </div>
      </form>
    </div>
  );
}
