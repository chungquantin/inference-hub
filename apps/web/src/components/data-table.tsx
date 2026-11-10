/** A plain, accessible table. Numeric columns are right-aligned with tabular figures. */
export function DataTable({
  caption,
  headers,
  rows,
  numeric = [],
}: {
  caption?: string;
  headers: React.ReactNode[];
  rows: { key: string; cells: React.ReactNode[] }[];
  numeric?: number[];
}) {
  const align = (i: number) => (numeric.includes(i) ? "text-right tabular-nums" : "text-left");
  return (
    <div className="overflow-x-auto">
      <table className="w-full text-sm">
        {caption && <caption className="mb-2 text-left text-xs text-zinc-500">{caption}</caption>}
        <thead>
          <tr className="border-b border-zinc-200 text-zinc-500 dark:border-zinc-800">
            {headers.map((h, i) => (
              <th key={i} scope="col" className={`whitespace-nowrap py-2 pr-4 font-normal ${align(i)}`}>
                {h}
              </th>
            ))}
          </tr>
        </thead>
        <tbody className="divide-y divide-zinc-100 dark:divide-zinc-900">
          {rows.map((row) => (
            <tr key={row.key}>
              {row.cells.map((cell, i) => (
                <td key={i} className={`py-2 pr-4 ${align(i)}`}>
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
