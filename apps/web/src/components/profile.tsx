import { Logo } from "@/components/ui/logo";

/** The top of an organization or product page: logo, name, a line of context and badges. */
export function ProfileHeader({
  name,
  logoUrl,
  logoName,
  subtitle,
  badges,
  actions,
}: {
  name: string;
  logoUrl?: string | null;
  /** Whose logo to show when it is not the page's own, such as a product's maker. */
  logoName?: string;
  subtitle?: React.ReactNode;
  badges?: React.ReactNode;
  actions?: React.ReactNode;
}) {
  return (
    <header className="flex flex-col gap-4 sm:flex-row sm:items-center">
      <Logo name={logoName ?? name} src={logoUrl} size="lg" />
      <div className="flex min-w-0 flex-1 flex-col gap-1">
        <h1 className="font-display text-[2rem] leading-tight font-semibold tracking-[-0.02em] text-ink">
          {name}
        </h1>
        {subtitle && <p className="text-[15px] text-ink-2">{subtitle}</p>}
        {badges && <div className="flex flex-wrap gap-1.5 pt-1">{badges}</div>}
      </div>
      {actions}
    </header>
  );
}

/** Key facts as a strip of labelled values on one surface. */
export function FactStrip({ facts }: { facts: { label: string; value: React.ReactNode }[] }) {
  const shown = facts.filter((f) => f.value != null && f.value !== "");
  if (shown.length === 0) return null;
  return (
    <dl className="grid grid-cols-2 overflow-hidden rounded-[10px] bg-surface shadow-elev-1 sm:grid-cols-4">
      {shown.map((fact) => (
        <div key={fact.label} className="border-line px-4 py-3 not-last:border-b sm:not-last:border-r sm:not-last:border-b-0">
          <dt className="text-xs text-ink-3">{fact.label}</dt>
          <dd className="mt-0.5 font-medium text-ink">{fact.value}</dd>
        </div>
      ))}
    </dl>
  );
}
