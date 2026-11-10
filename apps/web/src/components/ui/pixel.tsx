/**
 * Pixel components: the one place the design gets playful. Bars are drawn as
 * rows of square cells on an 8px grid, like boat.dev's character-cell charts,
 * so a column of prices reads at a glance.
 */

const CELLS = 24;

/**
 * A horizontal bar of square cells. `value / max` of the cells are filled;
 * the rest are drawn as empty ticks, so bars of different rows line up.
 */
export function PixelBar({
  value,
  max,
  cells = CELLS,
  tone = "muted",
  label,
}: {
  value: number;
  max: number;
  cells?: number;
  /** `lead` fills with the brand color, for the cheapest or the highlighted row. */
  tone?: "lead" | "muted";
  /** Accessible description, such as "$2.99 per GPU-hour". */
  label: string;
}) {
  const filled = max > 0 ? Math.max(1, Math.round((value / max) * cells)) : 0;
  const on = tone === "lead" ? "bg-s-7" : "bg-s-4";
  return (
    <span role="img" aria-label={label} className="pixelated flex gap-px" style={{ height: 8 }}>
      {Array.from({ length: cells }, (_, i) => (
        <span
          key={i}
          className={i < filled ? on : "bg-line"}
          style={{ width: 6, height: 8, flex: "none" }}
        />
      ))}
    </span>
  );
}

/**
 * A framed panel with a pixel-font title bar, like a window. Used for the
 * home page price boards and other "live data" panels.
 */
export function WindowFrame({
  title,
  meta,
  children,
  className = "",
}: {
  title: string;
  /** Right side of the title bar, such as an update time. */
  meta?: React.ReactNode;
  children: React.ReactNode;
  className?: string;
}) {
  return (
    <section className={`border border-ink bg-wash ${className}`}>
      <header className="flex h-6 items-center justify-between gap-3 bg-ink px-2 text-wash">
        <h2 className="font-pixel text-[10px] leading-none tracking-wide uppercase">{title}</h2>
        {meta && <span className="font-mono text-[11px] leading-none text-s-3">{meta}</span>}
      </header>
      {children}
    </section>
  );
}

/** The square bullet used in lists of facts. */
export function PixelBullet({ className = "bg-s-7" }: { className?: string }) {
  return <span aria-hidden className={`inline-block size-2 shrink-0 ${className}`} />;
}

/** A pulsing square that marks data as live. */
export function LiveDot() {
  return (
    <span aria-hidden className="relative inline-flex size-2">
      <span className="absolute inline-flex size-full animate-ping bg-s-5 opacity-60" />
      <span className="relative inline-flex size-2 bg-s-7" />
    </span>
  );
}
