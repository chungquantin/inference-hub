import { AnalyticsRail } from "@/components/analytics/rail";
import { api } from "@/lib/api";

export const revalidate = 300;

/** One home for every price, ranking, curve and signal: a rail on the left, the page on the right. */
export default async function AnalyticsLayout({ children }: { children: React.ReactNode }) {
  const { data } = await api.GET("/v1/market/metrics");
  return (
    // `data-wide` widens the page; see `body:has([data-wide])` in globals.css.
    <div data-wide className="grid gap-6 lg:grid-cols-[210px_minmax(0,1fr)] lg:gap-10">
      <AnalyticsRail metrics={data?.metrics ?? []} />
      <div className="min-w-0">{children}</div>
    </div>
  );
}
