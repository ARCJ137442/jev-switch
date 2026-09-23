import { useCallback, useEffect, useRef } from 'react';
import { useConflictControl } from '../app/feedback';

interface UseConfigConflictOptions<T> {
  /** GET 快照（mock 阶段深比较驱动；H3 换 mtime 检测时仅改这里的检测源） */
  fetchSnapshot: () => Promise<T>;
  /** 「重载」：把拉到的快照灌回页面状态 */
  applySnapshot: (snapshot: T) => void;
  /** 页面当前数据（「覆盖」时 PUT 用） */
  getCurrent: () => T;
  /** 「覆盖」= 用页面数据整表写回 */
  pushSnapshot: (current: T) => Promise<void>;
  /** 轮询间隔；0 = 不轮询 */
  pollMs?: number;
}

export interface ConfigConflictApi<T> {
  /** 拉取 + 应用 + 记 baseline（页面初始加载用） */
  sync: () => Promise<T>;
  /** 突变成功后以最新快照重置 baseline（清横幅） */
  noteBaseline: (snapshot: T) => void;
}

/**
 * 配置外部修改冲突（契约 04 §3）：
 * baseline = 上次 GET/PUT 的快照；轮询 GET 深比较，不一致 → 横幅
 * 重载（默认，弃 UI 数据）/ 覆盖（UI 写回）。禁止静默丢弃手改。
 *
 * 逻辑集中于此：H3（A7 mtime 上线）只需替换检测源与 baseline 记录方式，
 * banner / raise-clear 协议不变。
 */
export function useConfigConflict<T>(options: UseConfigConflictOptions<T>): ConfigConflictApi<T> {
  const { raiseConflict, clearConflict } = useConflictControl();
  const optsRef = useRef(options);
  optsRef.current = options;

  const baseline = useRef<string | null>(null);
  const raised = useRef(false);

  const ser = (v: T) => JSON.stringify(v);

  const dropBanner = useCallback(() => {
    if (raised.current) {
      raised.current = false;
      clearConflict();
    }
  }, [clearConflict]);

  const noteBaseline = useCallback(
    (snapshot: T) => {
      baseline.current = ser(snapshot);
      dropBanner();
    },
    [dropBanner],
  );

  const sync = useCallback(async (): Promise<T> => {
    const snapshot = await optsRef.current.fetchSnapshot();
    optsRef.current.applySnapshot(snapshot);
    baseline.current = ser(snapshot);
    return snapshot;
  }, []);

  // 稳定的 handlers（读 optsRef，不捕获过期页面状态；clearConflict 来自 useCallback 恒定引用）
  const handlersRef = useRef({
    reload: async () => {
      const snapshot = await optsRef.current.fetchSnapshot();
      optsRef.current.applySnapshot(snapshot);
      baseline.current = ser(snapshot);
      raised.current = false;
      clearConflict();
    },
    overwrite: async () => {
      await optsRef.current.pushSnapshot(optsRef.current.getCurrent());
      const snapshot = await optsRef.current.fetchSnapshot();
      optsRef.current.applySnapshot(snapshot);
      baseline.current = ser(snapshot);
      raised.current = false;
      clearConflict();
    },
  });

  const pollMs = options.pollMs ?? 0;
  useEffect(() => {
    if (pollMs <= 0) return;
    let cancelled = false;
    const tick = async () => {
      try {
        const snapshot = await optsRef.current.fetchSnapshot();
        if (cancelled) return;
        const s = ser(snapshot);
        if (baseline.current === null) {
          baseline.current = s; // 首次轮询只记 baseline
          return;
        }
        if (s !== baseline.current) {
          if (!raised.current) {
            raised.current = true;
            const handlers = handlersRef.current;
            raiseConflict({
              reload: () => handlers.reload(),
              overwrite: () => handlers.overwrite(),
            });
          }
        } else {
          dropBanner(); // 外部改动被撤销/已被覆盖 → 自动收横幅
        }
      } catch {
        // 轮询失败不打断：daemon 灯已负责可达性提示
      }
    };
    tick();
    const timer = window.setInterval(() => void tick(), pollMs);
    return () => {
      cancelled = true;
      window.clearInterval(timer);
      raised.current = false;
      clearConflict(); // 离开页面：横幅随页面数据作用域一起清
    };
    // raiseConflict/dropBanner 身份稳定（useCallback），不进 deps 防重挂
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [pollMs]);

  return { sync, noteBaseline };
}
