interface Option {
  value: string;
  label: string;
}

interface Props {
  options: Option[];
  value: string;
  onChange: (v: string) => void;
}

export function ModelSelect({ options, value, onChange }: Props) {
  return (
    <section className="rounded-lg border border-border bg-panel p-4">
      <header className="mb-3">
        <h2 className="text-sm font-semibold uppercase tracking-wider text-neutral-400">
          Model
        </h2>
      </header>
      <select
        className="w-full rounded border border-border bg-black/40 px-3 py-2 font-mono text-sm text-neutral-100 outline-none focus:border-accent"
        value={value}
        onChange={(e) => onChange(e.target.value)}
        aria-label="select model"
      >
        {options.map((opt) => (
          <option key={opt.value} value={opt.value}>
            {opt.label}
          </option>
        ))}
      </select>
      <p className="mt-2 text-xs text-neutral-500">
        Model selects which upstream receives the request. The router picks the provider based on
        the model id.
      </p>
    </section>
  );
}