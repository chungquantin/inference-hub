/**
 * A framed table in mono: 13/16 cells at 40px rows, numbers right-aligned with
 * tabular figures. Rows highlight on hover; one row can be marked (the lowest).
 */
export function DataTable({
  caption,
  headers,
  rows,
  numeric = [],
  highlight,
}: {
  caption?: React.ReactNode;
  headers: React.ReactNode[];
  rows: { key: string; cells: React.ReactNode[] }[];
  numeric?: number[];
  /** Key of a row to mark, such as the cheapest. */
  highlight?: string;
}) {
  const align = (i: number) => (numeric.includes(i) ? "text-right tabular" : "text-left");
  return (
    <div className="overflow-x-auto border border-line bg-wash">
      <table className="w-full border-collapse font-mono text-[13px] leading-4">
        {caption && (
          <caption className="border-b border-line px-3 py-2 text-left text-[12px] text-ink-3">
            {caption}
          </caption>
        )}
        <thead>
          <tr className="border-b border-line bg-paper text-ink-3">
            {headers.map((h, i) => (
              <th key={i} scope="col" className={`h-9 px-3 font-normal whitespace-nowrap ${align(i)}`}>
                {h}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {rows.map((row) => (
            <tr
              key={row.key}
              className={`border-b border-line last:border-0 ${row.key === highlight ? "bg-s-1" : "hover:bg-paper"}`}
            >
              {row.cells.map((cell, i) => (
                <td key={i} className={`h-11 px-3 align-middle ${align(i)}`}>
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
