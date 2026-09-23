/**
 * Routing 页（design/01 §6.2 模型路由 DAG / 二部图）— B1 仅落 IA 骨架，
 * 画布/边浮层/路由表在 B3 接线。
 */
export function RoutingPage() {
  return (
    <div className="mx-auto max-w-7xl px-6 py-8">
      <div className="mb-4 font-mono text-[10px] font-semibold uppercase tracking-widest text-inkSubtle">
        Routing
      </div>
      <section className="border border-border bg-panel p-6">
        <p className="text-sm text-inkMuted">还没有路由。</p>
        <p className="mt-2 text-sm text-inkSubtle">
          二部图连线编辑器（B3 施工）。
        </p>
      </section>
    </div>
  );
}
