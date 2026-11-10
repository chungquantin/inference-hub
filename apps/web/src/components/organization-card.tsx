import Link from "next/link";
import type { Organization } from "@inference-hub/api-client";
import { ORG_KIND_LABELS } from "@/lib/labels";

export function OrganizationCard({ org }: { org: Organization }) {
  return (
    <Link
      href={`/organizations/${org.slug}`}
      className="group flex flex-col gap-2 rounded-lg border border-zinc-200 p-4 transition-colors hover:border-zinc-400 dark:border-zinc-800 dark:hover:border-zinc-600"
    >
      <div className="flex items-baseline justify-between gap-2">
        <h3 className="font-medium group-hover:underline">{org.name}</h3>
        {org.hq_country && (
          <span className="font-mono text-xs text-zinc-500">{org.hq_country}</span>
        )}
      </div>
      {org.tagline && (
        <p className="line-clamp-2 text-sm text-zinc-600 dark:text-zinc-400">{org.tagline}</p>
      )}
      <ul className="mt-auto flex flex-wrap gap-1.5 pt-1">
        {org.kinds.map((kind) => (
          <li
            key={kind}
            className="rounded bg-zinc-100 px-1.5 py-0.5 text-xs text-zinc-700 dark:bg-zinc-800 dark:text-zinc-300"
          >
            {ORG_KIND_LABELS[kind]}
          </li>
        ))}
      </ul>
    </Link>
  );
}

export function OrganizationGrid({ orgs }: { orgs: Organization[] }) {
  if (orgs.length === 0) {
    return <p className="text-sm text-zinc-500">No organizations found.</p>;
  }
  return (
    <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
      {orgs.map((org) => (
        <OrganizationCard key={org.id} org={org} />
      ))}
    </div>
  );
}
