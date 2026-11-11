import Link from "next/link";

/**
 * A filter of an index page. Filters are plain GET form fields: they work
 * without JavaScript, live in the URL and can be shared or bookmarked.
 */
export type Filter =
  | { type: "search"; name: string; label: string; placeholder: string }
  | { type: "select"; name: string; label: string; options: { value: string; label: string }[]; any?: string }
  | { type: "number"; name: string; label: string; placeholder?: string; step?: string; min?: string; prefix?: string };

/** One click to a common view, such as "Under $0.50 / 1M". */
export type Preset = { label: string; params: Record<string, string> };

/**
 * The filter bar of an index: fields, an Apply button, presets, and chips for
 * every active filter that remove it in one click.
 */
export function FilterBar({
  action,
  filters,
  values,
  keep = {},
  presets = [],
  labels = {},
  describe = {},
}: {
  action: string;
  filters: Filter[];
  /** Current values, from the URL. */
  values: Record<string, string | undefined>;
  /** Parameters to carry over unchanged, such as `sort`. */
  keep?: Record<string, string | undefined>;
  presets?: Preset[];
  /** Chip labels per parameter, such as `max_price` → `Max price`. */
  labels?: Record<string, string>;
  /** Formats a value for its chip, such as `0.5` → `$0.50`. */
  describe?: Record<string, (value: string) => string>;
}) {
  const href = (params: Record<string, string | undefined>) => {
    const p = new URLSearchParams();
    for (const [k, v] of Object.entries(params)) if (v) p.set(k, v);
    const qs = p.toString();
    return qs ? `${action}?${qs}` : action;
  };
  const active = Object.entries(values).filter(([, v]) => v);

  return (
    <div className="flex flex-col gap-3">
      <form action={action} className="flex flex-wrap items-end gap-3 border border-line bg-paper p-3" role="search">
        {filters.map((f) => (
          <label key={f.name} className={`flex flex-col gap-1 ${f.type === "search" ? "min-w-[220px] flex-[2]" : "min-w-[130px] flex-1"}`}>
            <span className="font-mono text-[11px] text-ink-3 uppercase">{f.label}</span>
            {f.type === "select" ? (
              <select
                name={f.name}
                defaultValue={values[f.name] ?? ""}
                className="h-9 border border-line bg-wash px-2 font-mono text-[13px] text-ink focus:border-ink"
              >
                <option value="">{f.any ?? "Any"}</option>
                {f.options.map((o) => (
                  <option key={o.value} value={o.value}>
                    {o.label}
                  </option>
                ))}
              </select>
            ) : (
              <span className="flex h-9 items-center border border-line bg-wash focus-within:border-ink">
                {f.type === "search" && (
                  <span aria-hidden className="pl-2 font-mono text-[13px] text-s-7">
                    &gt;
                  </span>
                )}
                {f.type === "number" && f.prefix && (
                  <span aria-hidden className="pl-2 font-mono text-[13px] text-ink-3">
                    {f.prefix}
                  </span>
                )}
                <input
                  type={f.type === "number" ? "number" : "search"}
                  name={f.name}
                  defaultValue={values[f.name] ?? ""}
                  placeholder={f.placeholder}
                  step={f.type === "number" ? (f.step ?? "any") : undefined}
                  min={f.type === "number" ? (f.min ?? "0") : undefined}
                  className="h-full w-full min-w-0 bg-transparent px-2 font-mono text-[13px] text-ink outline-none placeholder:text-ink-3"
                />
              </span>
            )}
          </label>
        ))}
        {Object.entries(keep).map(([k, v]) => v && <input key={k} type="hidden" name={k} value={v} />)}
        <button type="submit" className="h-9 bg-ink px-4 text-[14px] text-wash hover:bg-s-8">
          Apply
        </button>
      </form>
      {(presets.length > 0 || active.length > 0) && (
        <div className="flex flex-wrap items-center gap-2">
          {active.map(([k, v]) => (
            <Link
              key={k}
              href={href({ ...values, ...keep, [k]: undefined })}
              className="inline-flex h-7 items-center gap-1.5 bg-s-7 px-2 font-mono text-[12px] text-wash hover:bg-s-8"
              aria-label={`Remove filter ${labels[k] ?? k}`}
            >
              {labels[k] ?? k}: {describe[k] ? describe[k](v!) : v}
              <span aria-hidden>✕</span>
            </Link>
          ))}
          {active.length > 1 && (
            <Link href={href(keep)} className="font-mono text-[12px] text-s-7 underline">
              Clear all
            </Link>
          )}
          {presets.length > 0 && <span className="ml-1 font-mono text-[11px] text-ink-3 uppercase">Try</span>}
          {presets.map((p) => (
            <Link
              key={p.label}
              href={href({ ...keep, ...p.params })}
              className="inline-flex h-7 items-center bg-field px-2 font-mono text-[12px] text-ink hover:bg-s-2"
            >
              {p.label}
            </Link>
          ))}
        </div>
      )}
    </div>
  );
}
