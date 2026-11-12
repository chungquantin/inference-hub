import Link from "next/link";
import { Logo } from "@/components/ui/logo";
import { creatorLogoUrl, creatorName, catalogCreator, modelLogoUrl } from "@/lib/catalog";
import creatorOrgs from "@/lib/creator-orgs.json";
import { gpuProducer } from "@/lib/gpu";
import { gpuHref, modelHref, orgHref } from "@/lib/hrefs";

type Size = "xs" | "sm" | "md";

/**
 * What a HyperLink points at. The `type` decides which other props exist, so
 * every kind of entity is linked and drawn the same way everywhere:
 *
 * - `org`       a company in the directory: a cloud, a lab, a gateway, an investor
 * - `creator`   the maker of a model, by the id a listing names (`deepseek-ai`);
 *               links to the company's page when the directory has one, else to its models
 * - `producer`  the company that makes a GPU, from the GPU's name (`H100 SXM` → NVIDIA)
 * - `gpu`       a GPU type's market page, with its producer's logo
 * - `model`     a model's market page, with its logo
 */
export type HyperLinkProps = (
  | { type: "org"; slug: string; name: string; logoUrl?: string | null }
  | { type: "creator"; maker: string | null | undefined }
  | { type: "producer"; gpuType: string }
  | { type: "gpu"; gpuType: string }
  | { type: "model"; id: string; maker?: string | null }
) & {
  size?: Size;
  /** Replaces the default link style, for chips and other framed forms. */
  className?: string;
  /** Text to show instead of the entity's name. */
  label?: React.ReactNode;
  /** A page other than the entity's default one, such as a curated product page. */
  href?: string;
};

const TEXT = "inline-flex min-w-0 items-center gap-2 font-sans text-[14px] text-ink hover:text-s-7 hover:underline";

type Resolved = { href: string; name: string; logo: string | null; title?: string } | null;

/** Where an entity links, what it is called and which logo goes with it. */
export function resolveHyperLink(props: HyperLinkProps): Resolved {
  switch (props.type) {
    case "org":
      return { href: orgHref(props.slug), name: props.name, logo: props.logoUrl ?? null };
    case "creator": {
      if (!props.maker) return null;
      const name = creatorName(props.maker) ?? props.maker;
      const orgSlug = (creatorOrgs as Record<string, string>)[catalogCreator(props.maker)?.id ?? ""];
      return {
        href: orgSlug ? orgHref(orgSlug) : `/market/models?maker=${encodeURIComponent(props.maker)}`,
        name,
        logo: creatorLogoUrl(props.maker),
        title: name === props.maker ? undefined : props.maker,
      };
    }
    case "producer": {
      const producer = gpuProducer(props.gpuType);
      return producer && { href: orgHref(producer.slug), name: producer.name, logo: producer.logo };
    }
    case "gpu":
      return { href: gpuHref(props.gpuType), name: props.gpuType, logo: gpuProducer(props.gpuType)?.logo ?? null };
    case "model":
      return { href: modelHref(props.id), name: props.id, logo: modelLogoUrl(props.id, props.maker) };
  }
}

/**
 * An entity as a link with its logo. Anything that names a company, a creator,
 * a GPU or a model should render through this, so it is always clickable and
 * always shows its logo (or a monogram when it has none).
 */
export function HyperLink(props: HyperLinkProps) {
  const found = resolveHyperLink(props);
  if (!found) return <span className="text-ink-3">—</span>;
  const { size = "sm", className, label, href } = props;
  return (
    <Link href={href ?? found.href} title={found.title} className={className ?? TEXT}>
      <Logo name={found.name} src={found.logo} size={size} />
      <span className="truncate">{label ?? found.name}</span>
    </Link>
  );
}
