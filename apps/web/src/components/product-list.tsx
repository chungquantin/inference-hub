import Link from "next/link";
import type { ProductSummary } from "@inference-hub/api-client";
import { HyperLink } from "@/components/hyper-link";
import { formatDate } from "@/lib/format";
import { PRODUCT_TYPE_LABELS } from "@/lib/labels";

/** A framed list of products, each with its maker's logo. */
export function ProductList({
  products,
  showMaker = true,
  empty = "No products listed yet.",
}: {
  products: ProductSummary[];
  showMaker?: boolean;
  empty?: string;
}) {
  if (products.length === 0) {
    return <p className="font-mono text-[13px] text-ink-3">{empty}</p>;
  }
  return (
    <ul className="border border-line bg-wash">
      {products.map((product) => (
        <li key={product.id} className="relative border-b border-line last:border-0 hover:bg-s-1">
          <div className="flex items-center gap-4 px-4 py-3">
            {showMaker && (
              <HyperLink
                type="org"
                slug={product.organization.slug}
                name={product.organization.name}
                logoUrl={product.organization.logo_url}
                size="md"
                label={<span className="sr-only">{product.organization.name}</span>}
                className="relative z-10 shrink-0"
              />
            )}
            <div className="min-w-0 flex-1">
              <p className="truncate text-[15px] text-ink">
                {/* The name link covers the whole row; the maker link sits above it. */}
                <Link href={`/products/${product.slug}`} className="hover:text-s-7 after:absolute after:inset-0">
                  {product.name}
                </Link>
                {showMaker && (
                  <HyperLink
                    type="org"
                    slug={product.organization.slug}
                    name={product.organization.name}
                    logoUrl={product.organization.logo_url}
                    size="xs"
                    className="relative z-10 ml-2 inline-flex items-center gap-1 align-middle font-mono text-[12px] text-ink-3 hover:text-s-7 hover:underline"
                  />
                )}
              </p>
              {product.tagline && <p className="truncate text-[14px] leading-5 text-ink-2">{product.tagline}</p>}
            </div>
            <div className="hidden shrink-0 flex-col items-end font-mono text-[12px] leading-4 text-ink-3 sm:flex">
              <span className="text-s-7">{PRODUCT_TYPE_LABELS[product.product_type]}</span>
              {product.launched_at && <span>{formatDate(product.launched_at)}</span>}
            </div>
          </div>
        </li>
      ))}
    </ul>
  );
}
