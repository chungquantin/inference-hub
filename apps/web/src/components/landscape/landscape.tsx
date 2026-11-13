"use client";

import Image from "next/image";
import Link from "next/link";
import { useEffect, useMemo, useRef, useState } from "react";
import type { LandscapeItem } from "@inference-hub/api-client";
import { ORG_KIND_LABELS } from "@/lib/labels";
import { GROUPS, type SortMode, compactMoney, formatRoundName, groupOf, headline, sortItems } from "@/lib/landscape";

const TILE_LOGO = 44;

function Mark({ item }: { item: LandscapeItem }) {
  if (item.logo_url) {
    return (
      <span
        className="inline-flex shrink-0 items-center justify-center overflow-hidden border border-line bg-wash"
        style={{ width: TILE_LOGO, height: TILE_LOGO }}
      >
        <Image src={item.logo_url} alt="" width={TILE_LOGO} height={TILE_LOGO} className="h-full w-full object-contain p-[12%]" />
      </span>
    );
  }
  return (
    <span
      aria-hidden
      className="inline-flex shrink-0 items-center justify-center border border-line bg-field font-mono text-[15px] font-medium text-ink-2"
      style={{ width: TILE_LOGO, height: TILE_LOGO }}
    >
      {item.name.slice(0, 1).toUpperCase()}
    </span>
  );
}

/**
 * The market on one page: every organization as a logo, in bands by what it does. A click opens
 * what we know about it, including every funding round with a source. `limit` shows only the
 * first tiles of each band (the landing page); the rest are on the full landscape.
 */
export function Landscape({
  items,
  totals,
  limit,
  fundedTotal,
}: {
  items: LandscapeItem[];
  /** Companies per band in the whole directory, when `items` holds only a part. */
  totals?: Record<string, number>;
  limit?: number;
  /** Companies with funding on record in the whole directory, when `items` holds only a part. */
  fundedTotal?: number;
}) {
  const preview = limit != null;
  const [q, setQ] = useState("");
  const [only, setOnly] = useState<string | null>(null);
  const [funding, setFunding] = useState(false);
  const [sort, setSort] = useState<SortMode>("valuation");
  const [open, setOpen] = useState<LandscapeItem | null>(null);

  const bands = useMemo(() => {
    const needle = q.trim().toLowerCase();
    const matches = needle
      ? items.filter((i) => `${i.name} ${i.summary ?? ""}`.toLowerCase().includes(needle))
      : items;
    const by = new Map<string, LandscapeItem[]>();
    for (const i of matches) {
      const g = groupOf(i);
      by.set(g.id, [...(by.get(g.id) ?? []), i]);
    }
    return GROUPS.filter((g) => by.has(g.id) && (!only || only === g.id)).map((g) => ({
      group: g,
      items: sortItems(by.get(g.id) ?? [], sort),
    }));
  }, [items, q, only, sort]);

  const counts = useMemo(() => {
    const c: Record<string, number> = {};
    for (const i of items) c[groupOf(i).id] = (c[groupOf(i).id] ?? 0) + 1;
    return c;
  }, [items]);
  const fundedCount = fundedTotal ?? items.filter((i) => i.rounds.length > 0).length;

  return (
    <div className="flex flex-col gap-5">
      <div className="flex flex-wrap items-end gap-x-6 gap-y-3">
        {!preview && (
          <label className="flex min-w-[220px] flex-1 flex-col gap-1 text-[13px] text-ink-3">
            Find a company
            <input
              type="search"
              value={q}
              onChange={(e) => setQ(e.target.value)}
              placeholder="deepseek, gpu cloud, coding agent…"
              className="h-10 border border-line-strong bg-wash px-3 text-[15px] text-ink outline-none focus-visible:border-s-7"
            />
          </label>
        )}
        {!preview && (
          <label className="flex flex-col gap-1 text-[13px] text-ink-3">
            Order by
            <select
              value={sort}
              onChange={(e) => setSort(e.target.value as SortMode)}
              className="h-10 border border-line-strong bg-wash px-3 text-[15px] text-ink"
            >
              <option value="valuation">Valuation</option>
              <option value="raised">Raised</option>
              <option value="name">Name</option>
            </select>
          </label>
        )}
        <button
          type="button"
          role="switch"
          aria-checked={funding}
          onClick={() => setFunding((f) => !f)}
          className="flex h-10 items-center gap-2.5 border border-line-strong bg-wash px-3 text-[14px] text-ink hover:bg-paper"
        >
          <span className={`relative h-4 w-8 transition-colors ${funding ? "bg-s-7" : "bg-line-strong"}`} aria-hidden>
            <span className={`absolute top-0.5 size-3 bg-wash transition-all ${funding ? "left-[18px]" : "left-0.5"}`} />
          </span>
          Show valuation and funding
          <span className="font-mono text-[12px] text-ink-3">{fundedCount} on record</span>
        </button>
      </div>

      {!preview && (
        <div className="flex flex-wrap gap-1.5" role="group" aria-label="Show a group">
          <Chip on={only == null} onClick={() => setOnly(null)}>
            All <span className="font-mono text-[12px] opacity-70">{items.length}</span>
          </Chip>
          {GROUPS.filter((g) => counts[g.id]).map((g) => (
            <Chip key={g.id} on={only === g.id} onClick={() => setOnly(only === g.id ? null : g.id)} color={g.color}>
              {g.label} <span className="font-mono text-[12px] opacity-70">{counts[g.id]}</span>
            </Chip>
          ))}
        </div>
      )}

      {bands.length === 0 && <p className="py-10 text-[15px] text-ink-3">Nothing matches “{q}”.</p>}

      <div className="grid gap-4 lg:grid-flow-dense lg:grid-cols-2">
        {bands.map(({ group, items: list }) => {
          const total = totals?.[group.id] ?? list.length;
          const shown = limit ? list.slice(0, limit) : list;
          const wide = total > 60;
          return (
            <section key={group.id} id={group.id} aria-labelledby={`${group.id}-h`} className={`scroll-mt-24 ${wide ? "lg:col-span-2" : "lg:self-start"}`}>
              <header className="flex items-baseline justify-between gap-4 px-4 py-2.5 text-white" style={{ background: group.color }}>
                <h3 id={`${group.id}-h`} className="font-display text-[18px] leading-6 font-medium tracking-[-0.03em]">
                  {group.label}
                </h3>
                {wide && <span className="hidden truncate text-[13px] opacity-85 sm:block">{group.blurb}</span>}
                <span className="font-mono text-[13px] whitespace-nowrap opacity-90">{total}</span>
              </header>
              <ul className="grid grid-cols-[repeat(auto-fill,minmax(88px,1fr))] gap-x-1 gap-y-2 border border-t-0 border-line bg-wash p-3">
                {shown.map((i) => (
                  <li key={i.slug}>
                    <Tile item={i} funding={funding} onOpen={() => setOpen(i)} />
                  </li>
                ))}
              </ul>
              {preview && total > shown.length && (
                <Link
                  href={`/landscape#${group.id}`}
                  className="block border border-t-0 border-line bg-paper px-4 py-2 text-[13px] font-medium text-s-7 hover:underline"
                >
                  {total - shown.length} more in {group.label.toLowerCase()}
                </Link>
              )}
            </section>
          );
        })}
      </div>

      <Detail item={open} onClose={() => setOpen(null)} />
    </div>
  );
}

function Chip({ on, onClick, color, children }: { on: boolean; onClick: () => void; color?: string; children: React.ReactNode }) {
  return (
    <button
      type="button"
      aria-pressed={on}
      onClick={onClick}
      className={`inline-flex h-9 items-center gap-1.5 border px-3 text-[14px] ${
        on ? "border-ink bg-ink text-wash" : "border-line-strong bg-wash text-ink-2 hover:bg-paper hover:text-ink"
      }`}
    >
      {color && <span className="size-2.5" style={{ background: color }} aria-hidden />}
      {children}
    </button>
  );
}

function Tile({ item, funding, onOpen }: { item: LandscapeItem; funding: boolean; onOpen: () => void }) {
  const h = headline(item);
  const caption = h.valuation ? compactMoney(h.valuation.amount ? h.valuation : h.valuation) : h.raised ? `${compactMoney(h.raised)} raised` : null;
  return (
    <button
      type="button"
      onClick={onOpen}
      title={item.name}
      className="group flex w-full flex-col items-center gap-1.5 px-1 py-1.5 text-center outline-none hover:bg-paper focus-visible:bg-s-1 focus-visible:outline-2 focus-visible:outline-s-7"
    >
      <Mark item={item} />
      <span className="line-clamp-2 text-[12px] leading-4 text-ink-2 group-hover:text-ink">{item.name}</span>
      {funding && caption && (
        <span className={`font-mono text-[11px] leading-none tabular ${h.valuation ? "font-medium text-s-8" : "text-ink-3"}`}>{caption}</span>
      )}
    </button>
  );
}

/** What we know about one organization, in a dialog: roles, summary, and each funding round with its source. */
function Detail({ item, onClose }: { item: LandscapeItem | null; onClose: () => void }) {
  const ref = useRef<HTMLDialogElement>(null);
  useEffect(() => {
    const d = ref.current;
    if (!d) return;
    if (item && !d.open) d.showModal();
    if (!item && d.open) d.close();
  }, [item]);
  const g = item ? groupOf(item) : null;
  const h = item ? headline(item) : {};
  return (
    <dialog
      ref={ref}
      onClose={onClose}
      onClick={(e) => e.target === ref.current && onClose()}
      aria-label={item?.name}
      className="m-auto w-[min(640px,calc(100vw-32px))] max-h-[85vh] overflow-y-auto border border-line-strong bg-wash p-0 text-ink backdrop:bg-black/40"
    >
      {item && g && (
        <div>
          <div className="h-1.5" style={{ background: g.color }} />
          <div className="flex items-start gap-4 p-5">
            <Mark item={item} />
            <div className="min-w-0 flex-1">
              <h2 className="font-display text-[26px] leading-8 font-medium tracking-[-0.04em]">{item.name}</h2>
              <p className="mt-1 flex flex-wrap gap-x-3 gap-y-1 text-[13px] text-ink-3">
                {item.kinds.map((k) => (
                  <span key={k}>{ORG_KIND_LABELS[k]}</span>
                ))}
                {item.hq_country && <span>{item.hq_country}</span>}
                {item.founded_year && <span>Founded {item.founded_year}</span>}
                {item.status !== "active" && <span className="text-hi">{item.status}</span>}
              </p>
            </div>
            <button
              type="button"
              onClick={onClose}
              aria-label="Close"
              className="size-9 border border-line-strong text-[18px] leading-none text-ink-2 hover:bg-paper"
            >
              ×
            </button>
          </div>
          {item.summary && <p className="px-5 pb-4 text-[15px] leading-6 text-ink-2">{item.summary}</p>}
          <div className="flex flex-wrap gap-x-5 gap-y-1 px-5 pb-5 text-[14px]">
            <Link href={`/organizations/${item.slug}`} className="font-medium text-s-7 hover:underline">
              Open the profile
            </Link>
            {item.website && (
              <a href={item.website} rel="noopener" target="_blank" className="text-ink-2 underline hover:text-s-7">
                {item.website.replace(/^https?:\/\/(www\.)?/, "")}
              </a>
            )}
          </div>

          <div className="border-t border-line px-5 py-4">
            {item.rounds.length === 0 ? (
              <p className="text-[14px] leading-6 text-ink-3">No funding on record. We list a round only with a source we can link.</p>
            ) : (
              <>
                <div className="mb-3 flex flex-wrap gap-x-8 gap-y-2">
                  {h.valuation && (
                    <Fact label="Latest valuation" value={compactMoney(h.valuation)} />
                  )}
                  {h.raised && <Fact label={`Raised in ${item.rounds.length} round${item.rounds.length === 1 ? "" : "s"}`} value={compactMoney(h.raised)} />}
                </div>
                <ol className="flex flex-col divide-y divide-line">
                  {item.rounds.map((r) => (
                    <li key={`${r.round}-${r.announced_on}`} className="flex flex-col gap-0.5 py-2.5 text-[14px]">
                      <div className="flex flex-wrap items-baseline justify-between gap-2">
                        <span className="font-medium">
                          {r.source_url ? (
                            <a href={r.source_url} rel="noopener" target="_blank" className="underline hover:text-s-7">
                              {formatRoundName(r.round)}
                            </a>
                          ) : (
                            formatRoundName(r.round)
                          )}
                          <span className="ml-2 font-normal text-ink-3">{r.announced_on}</span>
                        </span>
                        <span className="font-mono text-[13px] tabular">
                          {r.amount != null ? compactMoney({ amount: r.amount, currency: r.currency }) : "amount n/a"}
                          {r.valuation != null && (
                            <span className="ml-2 text-ink-3">at {compactMoney({ amount: r.valuation, currency: r.currency })}</span>
                          )}
                        </span>
                      </div>
                      {(r.leads.length > 0 || r.investors.length > 0) && (
                        <p className="text-[13px] leading-5 text-ink-3">
                          {r.leads.length > 0 && <>Led by {r.leads.join(", ")}. </>}
                          {r.investors.length > 0 && <>With {r.investors.join(", ")}.</>}
                        </p>
                      )}
                    </li>
                  ))}
                </ol>
              </>
            )}
          </div>
        </div>
      )}
    </dialog>
  );
}

function Fact({ label, value }: { label: string; value: string }) {
  return (
    <div>
      <span className="block font-display text-[24px] leading-7 font-semibold tracking-[-0.03em] tabular">{value}</span>
      <span className="text-[13px] text-ink-3">{label}</span>
    </div>
  );
}
