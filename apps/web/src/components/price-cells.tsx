import Link from "next/link";
import { Logo } from "@/components/ui/logo";
import { formatDate } from "@/lib/format";

/** A provider with its logo, linking to its page. */
export function ProviderLink({
  slug,
  name,
  logoUrl,
}: {
  slug: string;
  name: string;
  logoUrl?: string | null;
}) {
  return (
    <Link href={`/organizations/${slug}`} className="inline-flex items-center gap-2 font-medium text-ink hover:text-accent">
      <Logo name={name} src={logoUrl} size="sm" />
      {name}
    </Link>
  );
}

/** When a price was last checked, linking to its source when there is one. */
export function CheckedOn({ date, source }: { date: string; source?: string | null }) {
  const label = formatDate(date);
  return source ? (
    <a href={source} rel="noopener" target="_blank" className="text-ink-3 underline decoration-line-strong underline-offset-2 hover:text-ink">
      {label}
    </a>
  ) : (
    <span className="text-ink-3">{label}</span>
  );
}
