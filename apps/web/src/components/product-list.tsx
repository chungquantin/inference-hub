import Link from "next/link";
import type { ProductSummary } from "@inference-hub/api-client";
import { Logo } from "@/components/ui/logo";
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
        <li key={product.id} className="border-b border-line last:border-0">
          <Link href={`/products/${product.slug}`} className="group flex items-center gap-4 px-4 py-3 hover:bg-s-1">
            {showMaker && (
              <Logo name={product.organization.name} src={product.organization.logo_url} size="md" />
            )}
            <div className="min-w-0 flex-1">
              <p className="truncate text-[15px] text-ink group-hover:text-s-7">
                {product.name}
                {showMaker && (
                  <span className="ml-2 font-mono text-[12px] text-ink-3">{product.organization.name}</span>
                )}
              </p>
              {product.tagline && <p className="truncate text-[14px] leading-5 text-ink-2">{product.tagline}</p>}
            </div>
            <div className="hidden shrink-0 flex-col items-end font-mono text-[12px] leading-4 text-ink-3 sm:flex">
              <span className="text-s-7">{PRODUCT_TYPE_LABELS[product.product_type]}</span>
              {product.launched_at && <span>{formatDate(product.launched_at)}</span>}
            </div>
          </Link>
        </li>
      ))}
    </ul>
  );
}
