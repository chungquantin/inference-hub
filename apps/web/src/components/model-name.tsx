import Link from "next/link";
import { HyperLink } from "@/components/hyper-link";
import { Logo } from "@/components/ui/logo";
import { creatorName, modelLogoUrl } from "@/lib/catalog";
import { modelHref } from "@/lib/hrefs";

/**
 * A model in a table: its logo, the name as a link, and its creator below as
 * another link with its logo. Without a logo, the plate shows a monogram.
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
  /** The page of the model; the market page by default. */
  href?: string;
  maker?: string | null;
  /** A second line instead of the creator, such as the provider's own name. */
  subtitle?: React.ReactNode;
  showCreator?: boolean;
}) {
  const creator = maker ? creatorName(maker) : null;
  const link = "truncate text-ink underline decoration-line-strong hover:text-s-7";
  return (
    <span className="flex min-w-0 items-center gap-2.5">
      <Logo name={creator ?? id} src={modelLogoUrl(id, maker)} size="sm" />
      <span className="flex min-w-0 flex-col">
        <Link href={href ?? modelHref(id)} className={link}>
          {id}
        </Link>
        {subtitle ? (
          <span className="max-w-[300px] truncate text-[12px] text-ink-3">{subtitle}</span>
        ) : (
          showCreator &&
          maker && (
            <HyperLink
              type="creator"
              maker={maker}
              size="xs"
              className="inline-flex min-w-0 items-center gap-1 text-[12px] text-ink-3 hover:text-s-7 hover:underline"
            />
          )
        )}
      </span>
    </span>
  );
}
