/**
 * Providers 页（design/01 §6.1）— B1 仅落 IA 骨架，
 * 卡片/密钥/Probe/增删/冲突检测在 B2 接线。
 */
export function ProvidersPage() {
  return (
    <div className="mx-auto max-w-7xl px-6 py-8">
      <div className="mb-4 font-mono text-[10px] font-semibold uppercase tracking-widest text-inkSubtle">
        Providers
      </div>
      <section className="border border-border bg-panel p-6">
        <p className="text-sm text-inkMuted">还没有提供商。</p>
        <p className="mt-2 text-sm text-inkSubtle">
          粘贴 toml 片段或参考 rs/providers.example.toml 添加第一个上游（B2 施工）。
        </p>
      </section>
    </div>
  );
}
