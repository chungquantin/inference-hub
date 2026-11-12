import { Rail } from "@/components/terminal/rail";
import { api } from "@/lib/api";

export const revalidate = 300;

export default async function TerminalLayout({ children }: { children: React.ReactNode }) {
  const { data, error } = await api.GET("/v1/market/metrics");
  if (error || !data) throw new Error("Could not load the metrics");
  return (
    <div className="grid gap-6 lg:grid-cols-[210px_minmax(0,1fr)] lg:gap-10">
      <Rail metrics={data.metrics} />
      <div className="min-w-0">{children}</div>
    </div>
  );
}
