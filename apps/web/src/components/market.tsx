import Link from "next/link";
import { PixelBar } from "@/components/ui/pixel";
import { formatPrice } from "@/lib/format";

/** Path of a GPU type's market page. */
export function gpuHref(gpuType: string): string {
  return `/market/gpus/${encodeURIComponent(gpuType)}`;
}

/** Path of a model's market page, by its normalized key. */
export function modelHref(key: string): string {
  return `/market/models/${encodeURIComponent(key)}`;
}

/**
 * Where a price sits between the market's low and high, as a pixel bar with
 * the median marked: the shape of the spread at a glance.
 */
export function SpreadBar({
  min,
  median,
  max,
  unit,
}: {
  min: number;
  median: number;
  max: number;
  unit: string;
}) {
  return (
    <PixelBar
      value={median}
      max={max}
      cells={16}
      label={`${formatPrice(min)} to ${formatPrice(max)} ${unit}, median ${formatPrice(median)}`}
    />
  );
}

/** How many times the highest price is the lowest: the spread a buyer can save. */
export function spread(min: number, max: number): string {
  if (min <= 0) return "—";
  const x = max / min;
  return x >= 10 ? `${Math.round(x)}×` : `${x.toFixed(1)}×`;
}

/** A row link styled like the other table links. */
export function RowLink({ href, children }: { href: string; children: React.ReactNode }) {
  return (
    <Link href={href} className="text-ink underline decoration-line-strong hover:text-s-7">
      {children}
    </Link>
  );
}

/** "3 min ago", "5 h ago": how long before `now` (an ISO time) `iso` was. */
export function ago(iso: string | null | undefined, now: string | null | undefined): string {
  if (!iso) return "never";
  const ref = now ? new Date(now).getTime() : new Date(iso).getTime();
  const s = Math.max(0, Math.round((ref - new Date(iso).getTime()) / 1000));
  if (s < 90) return "just now";
  const m = Math.round(s / 60);
  if (m < 90) return `${m} min ago`;
  const h = Math.round(m / 60);
  if (h < 36) return `${h} h ago`;
  return `${Math.round(h / 24)} d ago`;
}
