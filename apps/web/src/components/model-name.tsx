import Link from "next/link";
import { Logo } from "@/components/ui/logo";
import { creatorLogoUrl, creatorName, modelLogoUrl } from "@/lib/catalog";

/** A creator: its logo and name, as the catalog knows it (`meta-llama` → Meta). */
export function CreatorBadge({ maker, size = "xs" }: { maker: string | null | undefined; size?: "xs" | "sm" }) {
  if (!maker) return <span className="text-ink-3">—</span>;
  const name = creatorName(maker) ?? maker;
  return (
    <span className="inline-flex min-w-0 items-center gap-1.5 text-ink-2" title={name === maker ? undefined : maker}>
      <Logo name={name} src={creatorLogoUrl(maker)} size={size} />
      <span className="truncate">{name}</span>
    </span>
  );
}

/**
 * A model in a table: its logo, the name as a link, and its creator below with
 * the creator's logo. Logos come from the inference catalog; without one, the
 * plate shows a monogram.
 */
export function ModelName({
  id,
  href,
  maker,
  subtitle,
  showCreator = true,
}: {
  /** The model id or market key, such as `deepseek-v3.2`. */
  id: string;
  href: string;
  maker?: string | null;
  /** A second line instead of the creator, such as the provider's own name. */
  subtitle?: React.ReactNode;
  showCreator?: boolean;
}) {
  const creator = maker ? creatorName(maker) : null;
  return (
    <span className="flex min-w-0 items-center gap-2.5">
      <Logo name={creator ?? id} src={modelLogoUrl(id, maker)} size="sm" />
      <span className="flex min-w-0 flex-col">
        <Link href={href} className="truncate text-ink underline decoration-line-strong hover:text-s-7">
          {id}
        </Link>
        {subtitle ? (
          <span className="max-w-[300px] truncate text-[12px] text-ink-3">{subtitle}</span>
        ) : (
          showCreator &&
          maker && (
            <span className="flex min-w-0 items-center gap-1 text-[12px] text-ink-3">
              <Logo name={creator ?? maker} src={creatorLogoUrl(maker)} size="xs" />
              <span className="truncate">{creator ?? maker}</span>
            </span>
          )
        )}
      </span>
    </span>
  );
}
