import Link from "next/link";
import { formatDate } from "@/lib/format";

/** A link to a provider's page. */
export function ProviderLink({ slug, name }: { slug: string; name: string }) {
  return (
    <Link href={`/organizations/${slug}`} className="font-medium hover:underline">
      {name}
    </Link>
  );
}

/** When a price was checked, linking to its source when there is one. */
export function CheckedOn({ date, source }: { date: string; source?: string | null }) {
  const label = formatDate(date);
  return source ? (
    <a href={source} rel="noopener" target="_blank" className="underline">
      {label}
    </a>
  ) : (
    <>{label}</>
  );
}
