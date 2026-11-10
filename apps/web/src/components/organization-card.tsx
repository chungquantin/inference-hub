import Link from "next/link";
import type { Organization } from "@inference-hub/api-client";
import { Logo } from "@/components/ui/logo";
import { ORG_KIND_LABELS } from "@/lib/labels";

export function OrganizationCard({ org }: { org: Organization }) {
  return (
    <Link
      href={`/organizations/${org.slug}`}
      className="group flex gap-3.5 rounded-[10px] bg-surface p-4 shadow-elev-1 transition-shadow hover:shadow-elev-2"
    >
      <Logo name={org.name} src={org.logo_url} size="lg" />
      <div className="flex min-w-0 flex-1 flex-col gap-1">
        <div className="flex items-baseline justify-between gap-2">
          <h3 className="truncate font-display font-semibold tracking-[-0.01em] text-ink group-hover:text-accent">
            {org.name}
          </h3>
          {org.hq_country && <span className="font-mono text-xs text-ink-3">{org.hq_country}</span>}
        </div>
        {org.tagline && <p className="line-clamp-2 text-sm text-ink-2">{org.tagline}</p>}
        <p className="mt-auto pt-1 text-xs text-ink-3">
          {org.kinds.map((kind) => ORG_KIND_LABELS[kind]).join(", ")}
        </p>
      </div>
    </Link>
  );
}

export function OrganizationGrid({ orgs }: { orgs: Organization[] }) {
  if (orgs.length === 0) {
    return <p className="text-sm text-ink-3">No organizations match. Try a broader search.</p>;
  }
  return (
    <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
      {orgs.map((org) => (
        <OrganizationCard key={org.id} org={org} />
      ))}
    </div>
  );
}
