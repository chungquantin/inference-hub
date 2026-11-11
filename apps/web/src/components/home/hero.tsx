"use client";

import Link from "next/link";
import { useRouter } from "next/navigation";
import { useEffect, useId, useMemo, useRef, useState } from "react";
import type { SearchHit } from "@inference-hub/api-client";

type Tile = { name: string; src: string };

/** Quick searches: each lands in the directory, already filtered. */
const QUICK = [
  { label: "H100", href: "/directory?tab=gpus&q=H100" },
  { label: "DeepSeek", href: "/directory?tab=models&q=DeepSeek" },
  { label: "Under $2", href: "/directory?tab=gpus&q=under%20%242" },
  { label: "Llama", href: "/directory?tab=models&q=Llama" },
];

const KIND: Record<string, string> = {
  model: "Model",
  media: "Media",
  gpu: "GPU",
  organization: "Company",
  product: "Product",
  news: "News",
};

/**
 * The home page's lead. A centered card holds the search; the page behind it
 * is a texture of real prices, and model makers (above) and the clouds that
 * run them (below) are wired into the card.
 */
export function Hero({
  providers,
  prices,
  makers,
  clouds,
}: {
  providers: number;
  /** Real price strings for the background texture. */
  prices: string[];
  makers: Tile[];
  clouds: Tile[];
}) {
  const router = useRouter();
  const stageRef = useRef<HTMLDivElement>(null);
  const cardRef = useRef<HTMLDivElement>(null);
  const topRef = useRef<HTMLDivElement>(null);
  const bottomRef = useRef<HTMLDivElement>(null);
  const [wires, setWires] = useState<{ d: string; x: number; y: number; len: number }[]>([]);
  const [size, setSize] = useState({ w: 0, h: 0 });

  const [q, setQ] = useState("");
  const [hits, setHits] = useState<SearchHit[]>([]);
  const [open, setOpen] = useState(false);
  const [active, setActive] = useState(0);
  const listId = useId();

  const texture = useMemo(() => buildTexture(prices), [prices]);

  // Wires: from each tile's edge to the card's edge, redrawn on resize.
  useEffect(() => {
    const stage = stageRef.current;
    if (!stage) return;
    const measure = () => {
      const box = stage.getBoundingClientRect();
      const card = cardRef.current?.getBoundingClientRect();
      if (!card) return;
      const out: { d: string; x: number; y: number; len: number }[] = [];
      const wire = (el: HTMLElement | null, top: boolean) => {
        if (!el) return;
        [...el.children].forEach((t, i, all) => {
          const r = (t as HTMLElement).getBoundingClientRect();
          const x = r.left + r.width / 2 - box.left;
          const y = (top ? r.bottom : r.top) - box.top;
          const tx = card.left - box.left + card.width * (0.25 + (0.5 * i) / Math.max(1, all.length - 1));
          const ty = (top ? card.top : card.bottom) - box.top;
          const my = (y + ty) / 2;
          const dir = top ? 1 : -1;
          const d = `M${x},${y} V${y + 14 * dir} Q${x},${my} ${(x + tx) / 2},${my} T${tx},${ty}`;
          out.push({ d, x: tx, y: ty, len: Math.abs(ty - y) + Math.abs(tx - x) + 40 });
        });
      };
      wire(topRef.current, true);
      wire(bottomRef.current, false);
      setSize({ w: box.width, h: box.height });
      setWires(out);
    };
    measure();
    const ro = new ResizeObserver(measure);
    ro.observe(stage);
    document.fonts?.ready.then(measure);
    return () => ro.disconnect();
  }, []);

  // Live suggestions from the same search the command palette uses.
  useEffect(() => {
    const term = q.trim();
    if (term.length < 2) return;
    const ctl = new AbortController();
    const t = setTimeout(() => {
      fetch(`/api/search?q=${encodeURIComponent(term)}`, { signal: ctl.signal })
        .then((r) => (r.ok ? r.json() : null))
        .then((d) => {
          if (d?.hits) {
            setHits((d.hits as SearchHit[]).slice(0, 6));
            setActive(0);
          }
        })
        .catch(() => {});
    }, 150);
    return () => {
      clearTimeout(t);
      ctl.abort();
    };
  }, [q]);

  // Short queries show nothing, whatever the last results were.
  const shown = q.trim().length >= 2 ? hits : [];

  const go = () => {
    const term = q.trim();
    if (open && shown[active]) router.push(shown[active].href);
    else router.push(term ? `/directory?q=${encodeURIComponent(term)}` : "/directory");
  };

  return (
    <section aria-labelledby="home-title" className="relative -mt-10 ml-[calc(50%-50vw)] w-screen overflow-hidden border-b border-line">
      <div
        aria-hidden
        className="pointer-events-none absolute inset-0 [mask-image:radial-gradient(ellipse_70%_80%_at_50%_50%,#000_30%,transparent_100%)]"
      >
        <pre className="absolute top-0 left-1/2 m-0 -translate-x-1/2 font-mono text-[14px] leading-[22px] font-medium tracking-[0.08em] whitespace-pre text-[#b9c2dc] opacity-75">
          {texture}
        </pre>
      </div>
      <div aria-hidden className="beams pointer-events-none absolute inset-0" />

      <div ref={stageRef} className="relative grid min-h-[720px] place-items-center px-4 py-28 sm:px-6">
        <svg
          aria-hidden
          className="pointer-events-none absolute inset-0 z-[1] size-full"
          viewBox={`0 0 ${size.w || 1} ${size.h || 1}`}
        >
          {wires.map((w, i) => (
            <g key={`${i}-${size.w}`}>
              <path
                d={w.d}
                className="wire"
                style={{ ["--len" as string]: w.len }}
                fill="none"
                stroke="var(--s-7)"
                strokeWidth="1.5"
                opacity="0.8"
              />
              <circle cx={w.x} cy={w.y} r="3.5" className="wire-end" fill="var(--s-7)" />
            </g>
          ))}
        </svg>

        <TileRow innerRef={topRef} tiles={makers} className="top-14" />
        <TileRow innerRef={bottomRef} tiles={clouds} className="bottom-14" />

        <div
          ref={cardRef}
          className="relative z-[3] w-full max-w-[520px] border border-line-strong bg-ground px-5 pt-7 pb-6 text-center sm:px-9 sm:pt-9"
        >
          <p className="mb-4 flex items-center justify-center gap-2.5 text-[14px] text-ink-3">
            <span aria-hidden className="size-[7px] rounded-full bg-lo" />
            <span>Live prices from {providers} providers</span>
          </p>
          <h1
            id="home-title"
            className="mb-6 font-display text-[clamp(34px,5.4vw,46px)] leading-[1.04] font-medium tracking-[-0.045em]"
          >
            Every inference price, side by side
          </h1>

          <form
            role="search"
            onSubmit={(e) => {
              e.preventDefault();
              go();
            }}
            className="relative"
          >
            <div className="flex h-[54px] items-center gap-2.5 border border-line-strong bg-wash px-3.5 text-left focus-within:border-s-7 focus-within:ring-[3px] focus-within:ring-s-2">
              <svg width="18" height="18" viewBox="0 0 18 18" fill="none" aria-hidden className="shrink-0 text-ink-3">
                <circle cx="8" cy="8" r="5.5" stroke="currentColor" strokeWidth="1.6" />
                <path d="M12.2 12.2L16 16" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" />
              </svg>
              <input
                type="text"
                name="q"
                value={q}
                autoComplete="off"
                spellCheck={false}
                placeholder="Search models, GPUs or companies"
                aria-label="Search models, GPUs or companies"
                role="combobox"
                aria-expanded={open && shown.length > 0}
                aria-controls={listId}
                aria-autocomplete="list"
                onChange={(e) => {
                  setQ(e.target.value);
                  setOpen(true);
                }}
                onFocus={() => setOpen(true)}
                onBlur={() => setTimeout(() => setOpen(false), 120)}
                onKeyDown={(e) => {
                  if (e.key === "ArrowDown") {
                    e.preventDefault();
                    setActive((a) => Math.min(shown.length - 1, a + 1));
                  } else if (e.key === "ArrowUp") {
                    e.preventDefault();
                    setActive((a) => Math.max(0, a - 1));
                  } else if (e.key === "Escape") setOpen(false);
                }}
                className="h-full min-w-0 flex-1 bg-transparent text-[16px] outline-none placeholder:text-ink-3"
              />
            </div>
            {open && shown.length > 0 && (
              <ul
                id={listId}
                role="listbox"
                className="absolute inset-x-0 top-[58px] z-10 border border-line-strong bg-wash text-left shadow-[0_20px_40px_-18px_rgba(31,30,26,0.3)]"
              >
                {shown.map((h, i) => (
                  <li key={`${h.kind}-${h.href}`} role="option" aria-selected={i === active}>
                    <Link
                      href={h.href}
                      onMouseEnter={() => setActive(i)}
                      className={`flex items-center gap-3 px-3.5 py-2.5 ${i === active ? "bg-s-1" : ""}`}
                    >
                      <span className="w-[68px] shrink-0 text-[12px] text-ink-3">{KIND[h.kind] ?? h.kind}</span>
                      <span className="min-w-0 flex-1 truncate text-[15px]">{h.title}</span>
                      {h.subtitle && <span className="hidden max-w-[40%] truncate text-[13px] text-ink-3 sm:inline">{h.subtitle}</span>}
                    </Link>
                  </li>
                ))}
              </ul>
            )}
            <button
              type="submit"
              className="mt-3 inline-flex h-12 w-full items-center justify-center bg-s-7 text-[15px] font-medium text-wash hover:bg-s-8"
            >
              Search prices
            </button>
          </form>

          <p className="mt-4 flex flex-wrap items-center justify-center gap-2 text-[14px] text-ink-3">
            Try
            {QUICK.map((c) => (
              <Link
                key={c.label}
                href={c.href}
                className="h-[30px] border border-line-strong bg-wash px-2.5 leading-[28px] text-ink-2 hover:border-ink hover:text-ink"
              >
                {c.label}
              </Link>
            ))}
          </p>
          <Link href="/directory" className="mt-4 inline-block text-[15px] underline underline-offset-4 hover:text-s-7">
            Browse the full directory
          </Link>
        </div>
      </div>
    </section>
  );
}

function TileRow({
  tiles,
  className,
  innerRef,
}: {
  tiles: Tile[];
  className: string;
  innerRef: React.RefObject<HTMLDivElement | null>;
}) {
  return (
    <div
      ref={innerRef}
      aria-hidden
      className={`absolute left-1/2 z-[2] flex w-[min(560px,86vw)] -translate-x-1/2 justify-between ${className}`}
    >
      {tiles.map((t) => (
        <span key={t.name} title={t.name} className="grid size-12 place-items-center border border-line-strong bg-wash p-[7px]">
          {/* eslint-disable-next-line @next/next/no-img-element -- tiny static logos */}
          <img src={t.src} alt="" className="size-full object-contain" />
        </span>
      ))}
    </div>
  );
}

/** A grid of characters: mostly spaces, symbols and the real prices passed in. Deterministic, so it hydrates cleanly. */
function buildTexture(prices: string[]): string {
  let seed = 7;
  const rnd = () => {
    seed = (seed * 16807) % 2147483647;
    return seed / 2147483647;
  };
  const sym = ["·", "+", "$", "#", "0", "*", "@", "-"];
  const cols = 190;
  const rows = 36;
  const pool = prices.length ? prices : ["1.19", "0.0207"];
  const out: string[] = [];
  for (let r = 0; r < rows; r++) {
    let line = "";
    while (line.length < cols) {
      const x = rnd();
      if (x < 0.5) line += " ".repeat(2 + Math.floor(rnd() * 5));
      else if (x < 0.7) line += `${sym[Math.floor(rnd() * sym.length)]} `;
      else line += `${pool[Math.floor(rnd() * pool.length)]} `;
    }
    out.push(line.slice(0, cols));
  }
  return out.join("\n");
}
