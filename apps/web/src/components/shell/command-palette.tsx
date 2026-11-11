"use client";

import { useRouter } from "next/navigation";
import { useCallback, useEffect, useId, useMemo, useRef, useState } from "react";
import type { SearchHit, SearchResults } from "@inference-hub/api-client";
import { NAV_LINKS } from "@/lib/nav";

const OPEN_EVENT = "ih:open-command-palette";
const RECENT_KEY = "ih:recent-searches";

/** Opens the palette from anywhere (the header button uses it). */
export function openCommandPalette() {
  window.dispatchEvent(new Event(OPEN_EVENT));
}

const GROUP_LABELS: Record<string, string> = {
  page: "Pages",
  recent: "Recent searches",
  organization: "Organizations",
  product: "Products",
  model: "Models",
  media: "Image, video, speech",
  gpu: "GPUs",
  news: "News",
};

/** Where "see all" goes for a kind, with the query. */
function seeAllHref(kind: string, q: string): string | undefined {
  const e = encodeURIComponent(q);
  switch (kind) {
    case "model":
      return `/market/models?q=${e}`;
    case "media":
      return `/market/media?q=${e}`;
    case "gpu":
      return `/market/gpus?q=${e}`;
    case "organization":
      return `/organizations?q=${e}`;
    case "product":
      return `/products?q=${e}`;
    case "news":
      return `/news?q=${e}`;
  }
}

type Item = {
  key: string;
  group: string;
  title: string;
  subtitle?: string | null;
  href: string;
  external?: boolean;
  logo?: string | null;
};

function readRecent(): string[] {
  try {
    const raw = JSON.parse(localStorage.getItem(RECENT_KEY) ?? "[]");
    return Array.isArray(raw) ? raw.filter((x) => typeof x === "string").slice(0, 5) : [];
  } catch {
    return [];
  }
}

function remember(q: string) {
  const next = [q, ...readRecent().filter((x) => x !== q)].slice(0, 5);
  try {
    localStorage.setItem(RECENT_KEY, JSON.stringify(next));
  } catch {
    /* private mode: no history */
  }
}

/**
 * Site-wide search: ⌘K, Ctrl+K or `/` from anywhere. Results come grouped by
 * kind as you type; arrows move, Enter opens, Escape closes. With no query it
 * lists recent searches and every page of the site.
 */
export function CommandPalette() {
  const router = useRouter();
  const [open, setOpen] = useState(false);
  const [q, setQ] = useState("");
  const [results, setResults] = useState<SearchResults | null>(null);
  const [loading, setLoading] = useState(false);
  const [active, setActive] = useState(0);
  const [recent, setRecent] = useState<string[]>([]);
  const inputRef = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLUListElement>(null);
  const restoreFocus = useRef<HTMLElement | null>(null);
  const listId = useId();

  const show = useCallback(() => {
    restoreFocus.current = document.activeElement as HTMLElement | null;
    setRecent(readRecent());
    setOpen(true);
  }, []);

  const close = useCallback(() => {
    setOpen(false);
    setQ("");
    setResults(null);
    setActive(0);
    restoreFocus.current?.focus?.();
  }, []);

  // Shortcuts and the open event.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const typing =
        e.target instanceof HTMLElement &&
        (e.target.isContentEditable || ["INPUT", "TEXTAREA", "SELECT"].includes(e.target.tagName));
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        if (open) close();
        else show();
      } else if (e.key === "/" && !typing && !open) {
        e.preventDefault();
        show();
      }
    };
    window.addEventListener("keydown", onKey);
    window.addEventListener(OPEN_EVENT, show);
    return () => {
      window.removeEventListener("keydown", onKey);
      window.removeEventListener(OPEN_EVENT, show);
    };
  }, [open, show, close]);

  useEffect(() => {
    if (open) inputRef.current?.focus();
  }, [open]);

  // Debounced search; stale responses are dropped.
  useEffect(() => {
    const term = q.trim();
    if (term.length < 2) return;
    const controller = new AbortController();
    const timer = setTimeout(async () => {
      setLoading(true);
      try {
        const res = await fetch(`/api/search?q=${encodeURIComponent(term)}`, {
          signal: controller.signal,
        });
        if (res.ok) {
          setResults(await res.json());
          setActive(0);
        }
      } catch {
        /* aborted or offline: keep the last results */
      } finally {
        if (!controller.signal.aborted) setLoading(false);
      }
    }, 150);
    return () => {
      clearTimeout(timer);
      controller.abort();
    };
  }, [q]);

  const term = q.trim();
  const items: Item[] = useMemo(() => {
    if (term.length < 2) {
      const pages = NAV_LINKS.filter(
        (l) => !term || l.label.toLowerCase().includes(term.toLowerCase()),
      ).map((l) => ({ key: `page:${l.href}`, group: "page", title: l.label, subtitle: l.description, href: l.href }));
      const recents = term
        ? []
        : recent.map((r) => ({
            key: `recent:${r}`,
            group: "recent",
            title: r,
            subtitle: "Search again",
            href: `/search?q=${encodeURIComponent(r)}`,
          }));
      return [...recents, ...pages];
    }
    const hits: Item[] = (results?.q === term ? results.hits : []).map((h: SearchHit) => ({
      key: `${h.kind}:${h.href}`,
      group: h.kind,
      title: h.title,
      subtitle: h.subtitle,
      href: h.href,
      external: h.kind === "news",
      logo: h.logo_url,
    }));
    return [
      ...hits,
      {
        key: "all",
        group: "all",
        title: `See all results for “${term}”`,
        href: `/search?q=${encodeURIComponent(term)}`,
      },
    ];
  }, [term, results, recent]);

  const go = useCallback(
    (item: Item) => {
      if (term.length >= 2) remember(term);
      close();
      if (item.external) window.open(item.href, "_blank", "noopener");
      else router.push(item.href);
    },
    [term, close, router],
  );

  // Keep the active row in view.
  useEffect(() => {
    listRef.current?.querySelector<HTMLElement>(`[data-index="${active}"]`)?.scrollIntoView({ block: "nearest" });
  }, [active]);

  if (!open) return null;

  const onKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      setActive((a) => Math.min(a + 1, items.length - 1));
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setActive((a) => Math.max(a - 1, 0));
    } else if (e.key === "Enter") {
      e.preventDefault();
      const item = items[active];
      if (item) go(item);
    } else if (e.key === "Escape") {
      e.preventDefault();
      close();
    } else if (e.key === "Tab") {
      // Focus stays in the input; Tab moves through results like arrows.
      e.preventDefault();
      setActive((a) => (e.shiftKey ? Math.max(a - 1, 0) : Math.min(a + 1, items.length - 1)));
    }
  };

  const counts = new Map((results?.q === term ? results.counts : []).map((c) => [c.kind, c.count]));
  let lastGroup = "";

  return (
    <div className="fixed inset-0 z-[60] flex items-start justify-center bg-ink/30 px-4 pt-[10vh]" onMouseDown={close}>
      <div
        role="dialog"
        aria-modal="true"
        aria-label="Search"
        className="flex max-h-[70vh] w-full max-w-[640px] flex-col border border-ink bg-wash"
        onMouseDown={(e) => e.stopPropagation()}
      >
        <div className="flex h-14 items-center gap-3 border-b border-line px-4">
          <span aria-hidden className="font-mono text-[15px] text-s-7">
            &gt;
          </span>
          <input
            ref={inputRef}
            value={q}
            onChange={(e) => {
              setQ(e.target.value);
              setActive(0);
              if (e.target.value.trim().length < 2) setResults(null);
            }}
            onKeyDown={onKeyDown}
            placeholder="Search models, GPUs, providers, news…"
            aria-label="Search"
            role="combobox"
            aria-expanded="true"
            aria-controls={listId}
            aria-activedescendant={items[active] ? `${listId}-${active}` : undefined}
            aria-autocomplete="list"
            className="h-full flex-1 bg-transparent font-mono text-[15px] text-ink outline-none placeholder:text-ink-3"
          />
          {loading && <span className="font-mono text-[12px] text-ink-3">searching…</span>}
          <kbd className="border border-line px-1.5 font-mono text-[11px] text-ink-3">esc</kbd>
        </div>
        <ul ref={listRef} id={listId} role="listbox" aria-label="Results" className="flex-1 overflow-y-auto py-1">
          {term.length >= 2 && results?.q === term && results.hits.length === 0 && (
            <li className="px-4 py-6 font-mono text-[13px] text-ink-3" role="presentation">
              Nothing matches “{term}”. Try fewer words or another spelling.
            </li>
          )}
          {items.map((item, i) => {
            const header = item.group !== lastGroup && item.group !== "all";
            lastGroup = item.group;
            const more = counts.get(item.group);
            const all = seeAllHref(item.group, term);
            return (
              <li key={item.key} role="presentation">
                {header && (
                  <div className="flex items-baseline justify-between px-4 pt-3 pb-1 font-mono text-[11px] text-ink-3 uppercase">
                    <span>{GROUP_LABELS[item.group] ?? item.group}</span>
                    {more && all && more > 5 && (
                      <a
                        href={all}
                        onMouseDown={(e) => e.preventDefault()}
                        onClick={(e) => {
                          e.preventDefault();
                          remember(term);
                          close();
                          router.push(all);
                        }}
                        className="normal-case text-s-7 underline"
                      >
                        all {more.toLocaleString("en-US")}
                      </a>
                    )}
                  </div>
                )}
                <div
                  id={`${listId}-${i}`}
                  data-index={i}
                  role="option"
                  aria-selected={i === active}
                  onMouseMove={() => setActive(i)}
                  onClick={() => go(item)}
                  className={`flex cursor-pointer items-center gap-3 px-4 py-2 ${
                    i === active ? "bg-s-1" : ""
                  } ${item.group === "all" ? "mt-1 border-t border-line font-mono text-[13px] text-s-7" : ""}`}
                >
                  {item.logo ? (
                    // eslint-disable-next-line @next/next/no-img-element
                    <img src={item.logo} alt="" width={20} height={20} className="size-5 shrink-0 border border-line object-cover" />
                  ) : (
                    item.group !== "all" && <span aria-hidden className="size-5 shrink-0 bg-field" />
                  )}
                  <span className="flex min-w-0 flex-1 flex-col">
                    <span className="truncate text-[14px] text-ink">{item.title}</span>
                    {item.subtitle && <span className="truncate text-[12px] text-ink-3">{item.subtitle}</span>}
                  </span>
                  {i === active && <span className="font-mono text-[11px] text-ink-3">↵</span>}
                </div>
              </li>
            );
          })}
        </ul>
        <div className="flex gap-4 border-t border-line px-4 py-2 font-mono text-[11px] text-ink-3">
          <span>↑↓ move</span>
          <span>↵ open</span>
          <span>esc close</span>
          <span className="ml-auto">⌘K or / anywhere</span>
        </div>
      </div>
    </div>
  );
}
