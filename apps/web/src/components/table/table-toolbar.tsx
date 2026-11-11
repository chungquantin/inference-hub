"use client";

import { useEffect, useRef, useState } from "react";

type Prefs = { hidden: string[]; compact: boolean };

function load(id: string): Prefs {
  try {
    const raw = JSON.parse(localStorage.getItem(`ih:table:${id}`) ?? "null");
    if (raw && Array.isArray(raw.hidden)) return { hidden: raw.hidden, compact: !!raw.compact };
  } catch {
    /* no preferences */
  }
  return { hidden: [], compact: false };
}

/**
 * The toolbar of a SmartTable: the caption, a column picker, a density toggle
 * and CSV export. Hidden columns are applied with a stylesheet scoped to the
 * table, so the server-rendered markup stays the same.
 */
export function TableToolbar({
  id,
  caption,
  columns,
  csv,
  csvName,
}: {
  id: string;
  caption?: React.ReactNode;
  columns: { id: string; header: string }[];
  csv?: string;
  csvName?: string;
}) {
  const [prefs, setPrefs] = useState<Prefs>({ hidden: [], compact: false });
  const [loaded, setLoaded] = useState(false);
  const [open, setOpen] = useState(false);
  const menuRef = useRef<HTMLDivElement>(null);

  // Preferences are per browser; read them after hydration.
  useEffect(() => {
    // eslint-disable-next-line react-hooks/set-state-in-effect -- localStorage is only readable on the client
    setPrefs(load(id));
    setLoaded(true);
  }, [id]);

  useEffect(() => {
    if (!loaded) return;
    try {
      localStorage.setItem(`ih:table:${id}`, JSON.stringify(prefs));
    } catch {
      /* private mode */
    }
    const table = document.querySelector(`[data-table="${id}"] table`);
    table?.setAttribute("data-density", prefs.compact ? "compact" : "comfortable");
  }, [id, prefs, loaded]);

  useEffect(() => {
    if (!open) return;
    const onDown = (e: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) setOpen(false);
    };
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && setOpen(false);
    document.addEventListener("mousedown", onDown);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDown);
      document.removeEventListener("keydown", onKey);
    };
  }, [open]);

  const toggle = (col: string) =>
    setPrefs((p) => ({
      ...p,
      hidden: p.hidden.includes(col) ? p.hidden.filter((c) => c !== col) : [...p.hidden, col],
    }));

  const download = () => {
    if (!csv) return;
    const url = URL.createObjectURL(new Blob([csv], { type: "text/csv;charset=utf-8" }));
    const a = document.createElement("a");
    a.href = url;
    a.download = `${csvName ?? id}.csv`;
    a.click();
    URL.revokeObjectURL(url);
  };

  const hiddenCss = prefs.hidden
    .map((c) => `[data-table="${id}"] [data-col="${CSS.escape(c)}"]{display:none}`)
    .join("");

  return (
    <div className="flex min-h-10 flex-wrap items-center gap-2 border-b border-line px-3 py-1.5">
      {hiddenCss && <style>{hiddenCss}</style>}
      <div className="mr-auto text-[13px] text-ink-3">{caption}</div>
      {columns.length > 0 && (
        <div className="relative" ref={menuRef}>
          <button
            type="button"
            onClick={() => setOpen((o) => !o)}
            aria-expanded={open}
            aria-haspopup="true"
            className="h-7 border border-line px-2 text-[13px] text-ink-2 hover:border-ink hover:text-ink"
          >
            Columns{prefs.hidden.length ? ` · ${prefs.hidden.length} hidden` : ""}
          </button>
          {open && (
            <div className="absolute top-full right-0 z-20 mt-1 w-56 border border-ink bg-wash py-1">
              {columns.map((c) => (
                <label key={c.id} className="flex cursor-pointer items-center gap-2 px-3 py-1.5 text-[13px] hover:bg-s-1">
                  <input
                    type="checkbox"
                    checked={!prefs.hidden.includes(c.id)}
                    onChange={() => toggle(c.id)}
                    className="accent-[var(--s-7)]"
                  />
                  {c.header}
                </label>
              ))}
              {prefs.hidden.length > 0 && (
                <button
                  type="button"
                  onClick={() => setPrefs((p) => ({ ...p, hidden: [] }))}
                  className="w-full border-t border-line px-3 pt-2 pb-1 text-left text-[13px] text-s-7 hover:underline"
                >
                  Show all columns
                </button>
              )}
            </div>
          )}
        </div>
      )}
      <button
        type="button"
        onClick={() => setPrefs((p) => ({ ...p, compact: !p.compact }))}
        aria-pressed={prefs.compact}
        className="h-7 border border-line px-2 text-[13px] text-ink-2 hover:border-ink hover:text-ink aria-pressed:border-[color:var(--s-7)] aria-pressed:text-s-7"
      >
        Compact
      </button>
      {csv && (
        <button
          type="button"
          onClick={download}
          className="h-7 border border-line px-2 text-[13px] text-ink-2 hover:border-ink hover:text-ink"
        >
          CSV
        </button>
      )}
    </div>
  );
}
