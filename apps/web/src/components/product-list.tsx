import Link from "next/link";
import type { ProductSummary } from "@inference-hub/api-client";
import { formatDate } from "@/lib/format";
import { PRODUCT_TYPE_LABELS } from "@/lib/labels";

/** A compact list of products, linking each to its page. */
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
    return <p className="text-sm text-zinc-500">{empty}</p>;
  }
  return (
    <ul className="divide-y divide-zinc-200 dark:divide-zinc-800">
      {products.map((product) => (
        <li key={product.id}>
          <Link
            href={`/products/${product.slug}`}
            className="group flex items-baseline justify-between gap-4 py-3"
          >
            <div className="min-w-0">
              <p className="font-medium group-hover:underline">
                {product.name}
                {showMaker && (
                  <span className="ml-2 text-sm font-normal text-zinc-500">
                    {product.organization.name}
                  </span>
                )}
              </p>
              {product.tagline && (
                <p className="truncate text-sm text-zinc-600 dark:text-zinc-400">
                  {product.tagline}
                </p>
              )}
            </div>
            <div className="flex shrink-0 flex-col items-end gap-0.5 text-xs text-zinc-500">
              <span className="font-mono">{PRODUCT_TYPE_LABELS[product.product_type]}</span>
              {product.launched_at && <span>{formatDate(product.launched_at)}</span>}
            </div>
          </Link>
        </li>
      ))}
    </ul>
  );
}
