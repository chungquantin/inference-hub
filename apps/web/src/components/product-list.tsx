import Link from "next/link";
import type { ProductSummary } from "@inference-hub/api-client";
import { Logo } from "@/components/ui/logo";
import { formatDate } from "@/lib/format";
import { PRODUCT_TYPE_LABELS } from "@/lib/labels";

/** A list of products, each with its maker's logo, linking to its page. */
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
    return <p className="text-sm text-ink-3">{empty}</p>;
  }
  return (
    <ul className="overflow-hidden rounded-[10px] bg-surface shadow-elev-1">
      {products.map((product) => (
        <li key={product.id} className="border-b border-line last:border-0">
          <Link
            href={`/products/${product.slug}`}
            className="group flex items-center gap-3 px-4 py-3 hover:bg-hover/50"
          >
            {showMaker && (
              <Logo name={product.organization.name} src={product.organization.logo_url} size="md" />
            )}
            <div className="min-w-0 flex-1">
              <p className="truncate font-medium text-ink group-hover:text-accent">
                {product.name}
                {showMaker && (
                  <span className="ml-2 text-sm font-normal text-ink-3">{product.organization.name}</span>
                )}
              </p>
              {product.tagline && <p className="truncate text-sm text-ink-2">{product.tagline}</p>}
            </div>
            <div className="hidden shrink-0 flex-col items-end gap-0.5 text-xs text-ink-3 sm:flex">
              <span>{PRODUCT_TYPE_LABELS[product.product_type]}</span>
              {product.launched_at && <span>{formatDate(product.launched_at)}</span>}
            </div>
          </Link>
        </li>
      ))}
    </ul>
  );
}
