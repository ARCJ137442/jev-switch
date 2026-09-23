import {
  createContext,
  useCallback,
  useContext,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from 'react';

/**
 * 全局反馈槽位（design/01 §5）：冲突横幅 + toast。
 * - 冲突：页面检测到外部修改时 raiseConflict(handlers)，Shell 渲染横幅；
 *   重载/覆盖动作由持数据的页面注入（B2 ProvidersPage 接线；H3 只换检测源）。
 * - toast：轻量提示，4s 自动消失；边框分隔、无阴影（design/01 §3.4）。
 */

/* ---------- 冲突横幅 ---------- */

export interface ConflictHandlers {
  /** 重载（默认）：放弃 UI 侧数据，重新拉取 */
  reload: () => void | Promise<void>;
  /** 覆盖：用 UI 侧数据写回 */
  overwrite: () => void | Promise<void>;
}

export interface ConflictControl {
  conflict: boolean;
  /** 当前挂起的冲突动作（Shell 渲染横幅时读取） */
  handlers: ConflictHandlers | null;
  raiseConflict: (handlers: ConflictHandlers) => void;
  clearConflict: () => void;
}

const ConflictContext = createContext<ConflictControl | null>(null);

/* ---------- toast ---------- */

export type ToastKind = 'ok' | 'warn' | 'danger';

interface ToastItem {
  id: number;
  kind: ToastKind;
  message: string;
}

export interface ToastApi {
  toast: (kind: ToastKind, message: string) => void;
  toasts: readonly ToastItem[];
}

const ToastContext = createContext<ToastApi | null>(null);

/* ---------- 组合 Provider（Shell 包住整棵子树） ---------- */

export function FeedbackProvider({ children }: { children: ReactNode }) {
  const [handlers, setHandlers] = useState<ConflictHandlers | null>(null);
  const [toasts, setToasts] = useState<ToastItem[]>([]);
  const nextId = useRef(1);

  const raiseConflict = useCallback((h: ConflictHandlers) => setHandlers(h), []);
  const clearConflict = useCallback(() => setHandlers(null), []);

  const toast = useCallback((kind: ToastKind, message: string) => {
    const id = nextId.current++;
    setToasts((prev) => [...prev, { id, kind, message }]);
    window.setTimeout(() => {
      setToasts((prev) => prev.filter((t) => t.id !== id));
    }, 4000);
  }, []);

  const conflictValue = useMemo<ConflictControl>(
    () => ({ conflict: handlers !== null, handlers, raiseConflict, clearConflict }),
    [handlers, raiseConflict, clearConflict],
  );
  const toastValue = useMemo<ToastApi>(() => ({ toast, toasts }), [toast, toasts]);

  return (
    <ConflictContext.Provider value={conflictValue}>
      <ToastContext.Provider value={toastValue}>{children}</ToastContext.Provider>
    </ConflictContext.Provider>
  );
}

/** 页面 / Shell 通用：冲突状态 + raise/clear（未包 Provider 时降级 no-op） */
export function useConflictControl(): ConflictControl {
  const ctx = useContext(ConflictContext);
  return (
    ctx ?? {
      conflict: false,
      handlers: null,
      raiseConflict: () => {},
      clearConflict: () => {},
    }
  );
}

export function useToast(): ToastApi {
  const ctx = useContext(ToastContext);
  return ctx ?? { toast: () => {}, toasts: [] };
}
