/** A table on a surface. Numeric columns are right-aligned with tabular figures. */
export function DataTable({
  caption,
  headers,
  rows,
  numeric = [],
  highlight,
}: {
  caption?: string;
  headers: React.ReactNode[];
  rows: { key: string; cells: React.ReactNode[] }[];
  numeric?: number[];
  /** Key of a row to mark, such as the cheapest. */
  highlight?: string;
}) {
  const align = (i: number) => (numeric.includes(i) ? "text-right tabular-nums" : "text-left");
  return (
    <div className="overflow-x-auto rounded-[10px] bg-surface shadow-elev-1">
      <table className="w-full text-sm">
        {caption && (
          <caption className="border-b border-line px-4 py-2.5 text-left text-xs text-ink-3">
            {caption}
          </caption>
        )}
        <thead>
          <tr className="border-b border-line text-ink-3">
            {headers.map((h, i) => (
              <th
                key={i}
                scope="col"
                className={`whitespace-nowrap px-4 py-2.5 text-xs font-medium ${align(i)}`}
              >
                {h}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {rows.map((row) => (
            <tr
              key={row.key}
              className={`border-b border-line last:border-0 ${row.key === highlight ? "bg-accent-tint" : "hover:bg-hover/50"}`}
            >
              {row.cells.map((cell, i) => (
                <td key={i} className={`px-4 py-2.5 align-middle ${align(i)}`}>
                  {cell}
                </td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
