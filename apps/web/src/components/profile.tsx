import { Logo } from "@/components/ui/logo";

/** The top of an organization or product page: big logo plate, name, context, badges. */
export function ProfileHeader({
  name,
  logoUrl,
  logoName,
  label,
  subtitle,
  badges,
  actions,
}: {
  name: string;
  logoUrl?: string | null;
  /** Whose logo to show when it is not the page's own, such as a product's maker. */
  logoName?: string;
  /** A mono line above the name, such as the breadcrumb. */
  label?: React.ReactNode;
  subtitle?: React.ReactNode;
  badges?: React.ReactNode;
  actions?: React.ReactNode;
}) {
  return (
    <header className="flex flex-col gap-6 border-b border-line pb-8">
      {label && <div className="font-mono text-[13px] leading-5 text-ink-3">{label}</div>}
      <div className="flex flex-col gap-5 sm:flex-row sm:items-center">
        <Logo name={logoName ?? name} src={logoUrl} size="lg" />
        <div className="flex min-w-0 flex-1 flex-col gap-1.5">
          <h1 className="font-display text-[40px] leading-[48px] tracking-[-0.02em] text-ink">{name}</h1>
          {subtitle && <p className="text-[16px] leading-6 text-ink-2">{subtitle}</p>}
        </div>
        {actions}
      </div>
      {badges && <div className="flex flex-wrap gap-1.5">{badges}</div>}
    </header>
  );
}

/** Key facts as a row of framed cells: mono label above, value below. */
export function FactStrip({ facts }: { facts: { label: string; value: React.ReactNode }[] }) {
  const shown = facts.filter((f) => f.value != null && f.value !== "");
  if (shown.length === 0) return null;
  return (
    <dl className="grid grid-cols-2 pt-px pl-px sm:grid-cols-3 lg:grid-cols-6">
      {shown.map((fact) => (
        <div key={fact.label} className="-mt-px -ml-px border border-line bg-wash px-4 py-3">
          <dt className="font-mono text-[12px] leading-4 text-ink-3">{fact.label}</dt>
          <dd className="mt-1 text-[15px] leading-6 text-ink">{fact.value}</dd>
        </div>
      ))}
    </dl>
  );
}
