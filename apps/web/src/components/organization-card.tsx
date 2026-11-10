import Link from "next/link";
import type { Organization } from "@inference-hub/api-client";
import { Logo } from "@/components/ui/logo";
import { ORG_KIND_LABELS } from "@/lib/labels";

export function OrganizationCard({ org }: { org: Organization }) {
  return (
    <Link
      href={`/organizations/${org.slug}`}
      className="group -mt-px -ml-px flex min-w-0 gap-4 border border-line bg-wash p-5 hover:z-10 hover:border-s-7"
    >
      <Logo name={org.name} src={org.logo_url} size="lg" />
      <div className="flex min-w-0 flex-1 flex-col gap-1">
        <div className="flex items-baseline justify-between gap-2">
          <h3 className="truncate font-display text-[18px] leading-6 tracking-[-0.01em] text-ink group-hover:text-s-7">
            {org.name}
          </h3>
          {org.hq_country && <span className="font-mono text-[12px] text-ink-3">{org.hq_country}</span>}
        </div>
        {org.tagline && <p className="line-clamp-2 text-[14px] leading-5 text-ink-2">{org.tagline}</p>}
        <p className="mt-auto pt-2 font-mono text-[12px] leading-4 text-s-7">
          {org.kinds.map((kind) => ORG_KIND_LABELS[kind]).join(" / ")}
        </p>
      </div>
    </Link>
  );
}

/** Cards on a shared grid: neighbouring frames overlap so every line is 1px. */
export function OrganizationGrid({ orgs }: { orgs: Organization[] }) {
  if (orgs.length === 0) {
    return <p className="font-mono text-[13px] text-ink-3">No organizations match. Try a broader search.</p>;
  }
  return (
    <div className="grid grid-cols-1 pt-px pl-px sm:grid-cols-2 lg:grid-cols-3">
      {orgs.map((org) => (
        <OrganizationCard key={org.id} org={org} />
      ))}
    </div>
  );
}
