import Link from "next/link";
import type { GpuPrice, ModelPrice } from "@inference-hub/api-client";
import { Logo } from "@/components/ui/logo";
import { PixelBar } from "@/components/ui/pixel";
import { formatPrice } from "@/lib/format";

/**
 * What an organization sells and at what price: model APIs by blended price,
 * GPUs by price per GPU-hour, each with a pixel bar scaled within its group.
 */
export function OrgPrices({ models, gpus }: { models: ModelPrice[]; gpus: GpuPrice[] }) {
  const maxModel = Math.max(...models.map((m) => m.blended_per_mtok ?? 0), 0);
  const maxGpu = Math.max(...gpus.map((g) => g.price_per_gpu_hour), 0);
  return (
    <div className="grid gap-px border border-line bg-line lg:grid-cols-2">
      {gpus.length > 0 && (
        <PriceGroup title="GPU instances" unit="per GPU-hour">
          {gpus.map((g) => (
            <Row
              key={`${g.gpu_type}:${g.gpu_count}:${g.region ?? ""}`}
              href={`/products/${g.product_slug}`}
              name={g.gpu_type}
              detail={[`${g.gpu_count}× GPU`, g.region].filter(Boolean).join(" / ")}
              value={g.price_per_gpu_hour}
              max={maxGpu}
              price={formatPrice(g.price_per_gpu_hour, g.currency)}
            />
          ))}
        </PriceGroup>
      )}
      {models.length > 0 && (
        <PriceGroup title="Model APIs" unit="blended per 1M tokens">
          {models.map((m) => (
            <Row
              key={m.product_slug}
              href={`/products/${m.product_slug}`}
              logo={<Logo name={m.maker.name} src={m.maker.logo_url} size="xs" />}
              name={m.product_name}
              detail={m.maker.name}
              value={m.blended_per_mtok ?? 0}
              max={maxModel}
              price={formatPrice(m.blended_per_mtok, m.currency)}
            />
          ))}
        </PriceGroup>
      )}
    </div>
  );
}

function PriceGroup({ title, unit, children }: { title: string; unit: string; children: React.ReactNode }) {
  return (
    <div className="bg-wash">
      <div className="flex items-baseline justify-between border-b border-line bg-paper px-4 py-2">
        <h3 className="font-mono text-[12px] text-ink">{title}</h3>
        <span className="font-mono text-[12px] text-ink-3">{unit}</span>
      </div>
      <ul>{children}</ul>
    </div>
  );
}

function Row({
  href,
  logo,
  name,
  detail,
  value,
  max,
  price,
}: {
  href: string;
  logo?: React.ReactNode;
  name: string;
  detail?: string;
  value: number;
  max: number;
  price: string;
}) {
  return (
    <li className="border-b border-line last:border-0">
      <Link
        href={href}
        className="group grid grid-cols-[minmax(0,1fr)_auto] items-center gap-x-4 gap-y-1.5 px-4 py-2.5 hover:bg-s-1 sm:grid-cols-[minmax(0,1fr)_112px_72px]"
      >
        <span className="flex min-w-0 items-center gap-2">
          {logo}
          <span className="truncate text-[14px] text-ink group-hover:text-s-7">{name}</span>
          {detail && <span className="truncate font-mono text-[12px] text-ink-3">{detail}</span>}
        </span>
        <span className="col-span-2 row-start-2 sm:col-span-1 sm:col-start-2 sm:row-start-1">
          <PixelBar value={value} max={max} cells={15} label={price} />
        </span>
        <span className="text-right font-mono text-[13px] text-ink tabular">{price}</span>
      </Link>
    </li>
  );
}
