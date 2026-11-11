/**
 * Data-visual atoms shared by the market pages: bars, framed panels and the
 * live dot. (The file keeps its old name; the exports are imported widely.)
 */

/**
 * A horizontal bar: `value / max` of the track is filled. `lead` fills with
 * the brand color for the cheapest or the highlighted row; the rest are a
 * quiet tint, so the leader stands out without a legend.
 */
export function PixelBar({
  value,
  max,
  tone = "muted",
  label,
  index = 0,
  animate = false,
}: {
  value: number;
  max: number;
  /** Kept for callers that used to set the cell count; bars are continuous now. */
  cells?: number;
  tone?: "lead" | "muted";
  /** Accessible description, such as "$2.99 per GPU-hour". */
  label: string;
  /** Position in the list, for the staggered grow-in. */
  index?: number;
  animate?: boolean;
}) {
  const pct = max > 0 ? Math.max(2, Math.round((value / max) * 100)) : 0;
  return (
    <span role="img" aria-label={label} className="block h-2 w-full overflow-hidden bg-field">
      <span
        className={`block h-full ${tone === "lead" ? "bg-lo" : "bg-s-4"} ${animate ? "bar-grow" : ""}`}
        style={{ width: `${pct}%`, ["--i" as string]: index }}
      />
    </span>
  );
}

/** A white sheet with a title row. Used for "live data" panels. */
export function WindowFrame({
  title,
  meta,
  children,
  className = "",
}: {
  title: string;
  /** Right side of the title row, such as a link to the full list. */
  meta?: React.ReactNode;
  children: React.ReactNode;
  className?: string;
}) {
  return (
    <section className={`border border-line bg-wash ${className}`}>
      <header className="flex h-11 items-center justify-between gap-3 border-b border-line px-4">
        <h2 className="text-[15px] leading-none font-semibold tracking-[-0.01em] text-ink">{title}</h2>
        {meta && <span className="text-[13px] leading-none text-s-7">{meta}</span>}
      </header>
      {children}
    </section>
  );
}

/** The square bullet used in lists of facts. */
export function PixelBullet({ className = "bg-s-7" }: { className?: string }) {
  return <span aria-hidden className={`inline-block size-2 shrink-0 ${className}`} />;
}

/** A pulsing dot that marks data as live. */
export function LiveDot() {
  return (
    <span aria-hidden className="relative inline-flex size-2">
      <span className="absolute inline-flex size-full animate-ping rounded-full bg-down opacity-50" />
      <span className="relative inline-flex size-2 rounded-full bg-down" />
    </span>
  );
}
