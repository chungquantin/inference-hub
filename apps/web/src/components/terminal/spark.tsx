/** A line of the last values, no axes. Rounded so server and browser markup match. */
export function Spark({
  values,
  width = 96,
  height = 24,
  tone = "var(--s-7)",
}: {
  values: number[];
  width?: number;
  height?: number;
  tone?: string;
}) {
  // One reading is no trend: leave the cell quiet until a second day arrives.
  if (values.length < 2) return <span className="text-ink-3" aria-label="No history yet">—</span>;
  const min = Math.min(...values);
  const max = Math.max(...values);
  const span = max - min || 1;
  const r = (n: number) => Math.round(n * 10) / 10;
  const pts = values.map(
    (v, i) => `${r((i / (values.length - 1)) * (width - 2) + 1)},${r(height - 2 - ((v - min) / span) * (height - 4))}`,
  );
  return (
    <svg width={width} height={height} viewBox={`0 0 ${width} ${height}`} aria-hidden className="block">
      <polyline points={pts.join(" ")} fill="none" stroke={tone} strokeWidth={1.5} strokeLinejoin="round" />
    </svg>
  );
}
