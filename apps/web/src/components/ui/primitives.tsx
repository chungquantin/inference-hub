import Link from "next/link";
import { Breadcrumbs } from "@/components/shell/breadcrumbs";

/** A white panel with a 1px hairline frame. The basic grouping surface. */
export function Frame({
  children,
  className = "",
  as: Tag = "div",
}: {
  children: React.ReactNode;
  className?: string;
  as?: "div" | "section" | "article" | "ul";
}) {
  return <Tag className={`border border-line bg-wash ${className}`}>{children}</Tag>;
}

/** Page title with an optional lead paragraph and actions on the right. */
export function PageHeader({
  title,
  lead,
  label,
  crumbs,
  logo,
  children,
}: {
  title: React.ReactNode;
  lead?: React.ReactNode;
  /** A short mono line above the title naming the section, such as "Compare". */
  label?: string;
  /** Breadcrumbs, shown instead of the label: parent pages first. */
  crumbs?: { href?: string; label: string }[];
  /** A logo shown before the title, such as a model's. */
  logo?: React.ReactNode;
  children?: React.ReactNode;
}) {
  return (
    <header className="flex flex-col gap-4 border-b border-line pb-8">
      {crumbs ? (
        <Breadcrumbs items={crumbs} />
      ) : (
        label && <p className="font-mono text-[13px] leading-5 text-s-7">{label}</p>
      )}
      <div className="flex flex-wrap items-end justify-between gap-6">
        <span className="flex min-w-0 items-center gap-4">
          {logo}
          <h1 className="min-w-0 font-display text-[40px] leading-[48px] font-normal tracking-[-0.02em] break-words text-ink">
            {title}
          </h1>
        </span>
        {children}
      </div>
      {lead && <div className="max-w-[62ch] text-[16px] leading-6 text-ink-2">{lead}</div>}
    </header>
  );
}

/** A section heading with an optional link to the full list. */
export function SectionHeader({
  title,
  href,
  linkLabel,
  count,
}: {
  title: string;
  href?: string;
  linkLabel?: string;
  count?: number;
}) {
  return (
    <div className="flex items-baseline justify-between gap-4">
      <h2 className="font-display text-[24px] leading-8 font-normal tracking-[-0.02em] text-ink">
        {title}
        {count != null && <span className="ml-2 font-mono text-sm text-ink-3 tabular">{count}</span>}
      </h2>
      {href && (
        <Link href={href} className="font-mono text-[13px] text-s-7 underline hover:text-s-9">
          {linkLabel ?? "View all"}
        </Link>
      )}
    </div>
  );
}

type Tone = "neutral" | "brand" | "solid" | "up" | "down" | "warn";
const TONES: Record<Tone, string> = {
  neutral: "bg-field text-ink-2",
  brand: "bg-s-1 text-s-8",
  solid: "bg-s-7 text-wash",
  up: "bg-up-tint text-up",
  down: "bg-down-tint text-down",
  warn: "bg-warn-tint text-warn",
};

/** A small square label, such as a kind or a status. */
export function Badge({ children, tone = "neutral" }: { children: React.ReactNode; tone?: Tone }) {
  return (
    <span className={`inline-flex h-5 items-center px-1.5 font-mono text-[11px] leading-none ${TONES[tone]}`}>
      {children}
    </span>
  );
}

/** A square filter toggle. The active one is filled with the brand color. */
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
          ? "inline-flex h-8 items-center bg-s-7 px-3 font-mono text-[13px] text-wash hover:bg-s-8"
          : "inline-flex h-8 items-center bg-field px-3 font-mono text-[13px] text-ink hover:bg-s-2"
      }
    >
      {children}
    </Link>
  );
}

const BUTTON =
  "inline-flex h-10 items-center justify-center gap-2 px-5 text-[15px] transition-colors disabled:opacity-50";

/** Buttons: solid for the main action, line for the rest. Square, no shadows. */
export function buttonClass(variant: "primary" | "secondary" = "primary"): string {
  return variant === "primary"
    ? `${BUTTON} bg-s-7 text-wash hover:bg-s-8 active:bg-s-9`
    : `${BUTTON} bg-wash text-s-7 shadow-[inset_0_0_0_1px_var(--s-7)] hover:bg-s-1`;
}

/** A price or count in mono with tabular figures. */
export function Price({ children, strong = false }: { children: React.ReactNode; strong?: boolean }) {
  return (
    <span className={`font-mono tabular ${strong ? "font-medium text-ink" : "text-ink-2"}`}>
      {children}
    </span>
  );
}

/** A big number with a mono caption, for summary strips. */
export function Stat({ value, label, href }: { value: React.ReactNode; label: string; href?: string }) {
  const body = (
    <>
      <span className="block font-display text-[32px] leading-10 tracking-[-0.02em] text-ink tabular">
        {value}
      </span>
      <span className="block font-mono text-[13px] leading-5 text-ink-3">{label}</span>
    </>
  );
  return href ? (
    <Link href={href} className="group block px-5 py-4 hover:bg-s-1">
      {body}
    </Link>
  ) : (
    <div className="px-5 py-4">{body}</div>
  );
}
