import Link from "next/link";

/** A white panel with the soft hairline elevation used for every grouped block. */
export function Surface({
  children,
  className = "",
  as: Tag = "div",
}: {
  children: React.ReactNode;
  className?: string;
  as?: "div" | "section" | "article";
}) {
  return <Tag className={`rounded-[10px] bg-surface shadow-elev-1 ${className}`}>{children}</Tag>;
}

/** Page title with an optional lead paragraph and actions on the right. */
export function PageHeader({
  title,
  lead,
  children,
}: {
  title: React.ReactNode;
  lead?: React.ReactNode;
  children?: React.ReactNode;
}) {
  return (
    <header className="flex flex-col gap-3 pb-2">
      <div className="flex flex-wrap items-end justify-between gap-4">
        <h1 className="font-display text-[2rem] leading-tight font-semibold tracking-[-0.02em] text-ink">
          {title}
        </h1>
        {children}
      </div>
      {lead && <p className="max-w-[65ch] text-[15px] leading-relaxed text-ink-2">{lead}</p>}
    </header>
  );
}

/** A section heading with an optional link to the full list. */
export function SectionHeader({
  title,
  href,
  linkLabel,
}: {
  title: string;
  href?: string;
  linkLabel?: string;
}) {
  return (
    <div className="flex items-baseline justify-between gap-4">
      <h2 className="font-display text-lg font-semibold tracking-[-0.01em] text-ink">{title}</h2>
      {href && (
        <Link href={href} className="text-sm text-ink-2 hover:text-accent">
          {linkLabel ?? "View all"}
        </Link>
      )}
    </div>
  );
}

type Tone = "neutral" | "accent" | "amber" | "red";
const TONES: Record<Tone, string> = {
  neutral: "bg-inset text-ink-2",
  accent: "bg-accent-tint text-accent-ink",
  amber: "bg-amber-tint text-amber",
  red: "bg-red-tint text-red",
};

/** A small non-interactive label, such as a kind or a status. */
export function Badge({ children, tone = "neutral" }: { children: React.ReactNode; tone?: Tone }) {
  return (
    <span className={`inline-flex items-center rounded-full px-2 py-0.5 text-xs font-medium ${TONES[tone]}`}>
      {children}
    </span>
  );
}

/** A filter pill. Active pills use the accent; `aria-current` marks the active one. */
export function Chip({
  href,
  active,
  children,
}: {
  href: string;
  active: boolean;
  children: React.ReactNode;
}) {
  return (
    <Link
      href={href}
      aria-current={active ? "page" : undefined}
      className={
        active
          ? "inline-flex h-8 items-center rounded-full bg-ink px-3 text-sm font-medium text-canvas"
          : "inline-flex h-8 items-center rounded-full bg-surface px-3 text-sm text-ink-2 shadow-elev-1 hover:text-ink"
      }
    >
      {children}
    </Link>
  );
}

const BUTTON =
  "inline-flex h-9 items-center justify-center gap-2 rounded-[6px] px-4 text-sm font-medium transition-colors disabled:opacity-50";

/** Buttons: primary for the main action of a view, secondary otherwise. */
export function buttonClass(variant: "primary" | "secondary" = "primary"): string {
  return variant === "primary"
    ? `${BUTTON} bg-accent text-on-accent hover:bg-accent-ink`
    : `${BUTTON} bg-surface text-ink shadow-elev-1 hover:bg-hover`;
}

/** A price in mono, with tabular figures, large enough to scan a column of them. */
export function Price({ children, strong = false }: { children: React.ReactNode; strong?: boolean }) {
  return (
    <span className={`font-mono tabular-nums ${strong ? "font-medium text-ink" : "text-ink-2"}`}>
      {children}
    </span>
  );
}
