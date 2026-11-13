import { Landscape } from "@/components/landscape/landscape";
import { api } from "@/lib/api";
import { groupOf, sortItems } from "@/lib/landscape";

/**
 * The map of the market. With `limit`, each band shows only its first tiles (the most valuable,
 * then the rest by name), so the landing page stays light and links to the full landscape.
 */
export async function LandscapeData({ limit }: { limit?: number }) {
  const { data, error } = await api.GET("/v1/landscape");
  if (error || !data) throw new Error("Could not load the landscape");
  if (!limit) return <Landscape items={data.items} />;

  const bands = new Map<string, typeof data.items>();
  for (const i of data.items) bands.set(groupOf(i).id, [...(bands.get(groupOf(i).id) ?? []), i]);
  const totals: Record<string, number> = {};
  const items = [...bands.entries()].flatMap(([id, list]) => {
    totals[id] = list.length;
    return sortItems(list, "market").slice(0, limit);
  });
  const fundedTotal = data.items.filter((i) => i.rounds.length > 0).length;
  return <Landscape items={items} totals={totals} limit={limit} fundedTotal={fundedTotal} />;
}
