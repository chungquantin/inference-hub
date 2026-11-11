import Link from "next/link";
import { TableToolbar } from "@/components/table/table-toolbar";

/**
 * A column of a {@link SmartTable}. Columns are declared once: header, cell,
 * alignment, sort key, CSV value and whether they can be hidden.
 */
export type Column<T> = {
  id: string;
  header: string;
  /** The cell. */
  cell: (row: T) => React.ReactNode;
  /** Plain value for CSV export; omit to leave the column out of the file. */
  csv?: (row: T) => string | number | null | undefined;
  numeric?: boolean;
  /** Server sort key; the header becomes a sort link. */
  sort?: string;
  /** Natural direction of the sort (prices ascend, counts descend). */
  sortDir?: "asc" | "desc";
  /** Hidden by default on narrow screens and in the column picker. */
  optional?: boolean;
  /** A hint shown on the header. */
  title?: string;
  className?: string;
};

export type SortState = { sort?: string; dir?: "asc" | "desc" };

/**
 * A data table for large, sortable lists. Sorting and paging live in the URL
 * (server-rendered, shareable); hiding columns, density and CSV export are
 * client-side extras in the toolbar. The header sticks while scrolling.
 */
export function SmartTable<T>({
  id,
  caption,
  columns,
  rows,
  rowKey,
  sort,
  sortHref,
  highlight,
  csvName,
  empty = "Nothing matches these filters.",
}: {
  /** Stable id: stores column and density preferences per table. */
  id: string;
  caption?: React.ReactNode;
  columns: Column<T>[];
  rows: T[];
  rowKey: (row: T) => string;
  sort?: SortState;
  /** The URL for a sort, keeping the other filters. */
  sortHref?: (sort: string, dir: "asc" | "desc") => string;
  /** Key of a row to mark, such as the cheapest. */
  highlight?: string;
  csvName?: string;
  empty?: React.ReactNode;
}) {
  const csvColumns = columns.filter((c) => c.csv);
  const csv = csvName
    ? [
        csvColumns.map((c) => c.header).join(","),
        ...rows.map((r) =>
          csvColumns
            .map((c) => {
              const v = c.csv!(r);
              if (v == null) return "";
              const s = String(v);
              return /[",\n]/.test(s) ? `"${s.replace(/"/g, '""')}"` : s;
            })
            .join(","),
        ),
      ].join("\n")
    : undefined;

  return (
    <div className="flex flex-col border border-line bg-wash" data-table={id}>
      <TableToolbar
        id={id}
        caption={caption}
        columns={columns.filter((c) => c.optional).map((c) => ({ id: c.id, header: c.header }))}
        csv={csv}
        csvName={csvName}
      />
      {rows.length === 0 ? (
        <p className="px-4 py-10 text-center font-mono text-[13px] text-ink-3">{empty}</p>
      ) : (
        <div className="max-h-[75vh] overflow-auto">
          <table className="w-full border-collapse font-mono text-[13px] leading-4">
            <thead className="sticky top-0 z-10">
              <tr className="bg-paper text-ink-3 shadow-[inset_0_-1px_0_var(--line)]">
                {columns.map((c) => {
                  const active = sort?.sort === c.sort && c.sort !== undefined;
                  const dir = active ? sort?.dir ?? c.sortDir ?? "asc" : undefined;
                  const next: "asc" | "desc" = active
                    ? dir === "asc"
                      ? "desc"
                      : "asc"
                    : (c.sortDir ?? "asc");
                  return (
                    <th
                      key={c.id}
                      scope="col"
                      data-col={c.id}
                      title={c.title}
                      aria-sort={active ? (dir === "asc" ? "ascending" : "descending") : undefined}
                      className={`h-9 px-3 font-normal whitespace-nowrap ${c.numeric ? "text-right" : "text-left"} ${c.className ?? ""}`}
                    >
                      {c.sort && sortHref ? (
                        <Link
                          href={sortHref(c.sort, next)}
                          scroll={false}
                          className={`inline-flex items-center gap-1 hover:text-ink ${active ? "text-ink" : ""}`}
                        >
                          {c.header}
                          <SortMark dir={dir} />
                        </Link>
                      ) : (
                        c.header
                      )}
                    </th>
                  );
                })}
              </tr>
            </thead>
            <tbody>
              {rows.map((r) => {
                const key = rowKey(r);
                return (
                  <tr
                    key={key}
                    className={`border-b border-line last:border-0 ${key === highlight ? "bg-s-1" : "hover:bg-paper"}`}
                  >
                    {columns.map((c) => (
                      <td
                        key={c.id}
                        data-col={c.id}
                        className={`h-11 px-3 align-middle ${
                          c.numeric ? "text-right tabular" : "text-left"
                        } ${c.className ?? ""}`}
                      >
                        {c.cell(r)}
                      </td>
                    ))}
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}

function SortMark({ dir }: { dir?: "asc" | "desc" }) {
  return (
    <svg width="8" height="10" viewBox="0 0 8 10" aria-hidden className="shrink-0">
      <path d="M4 0L8 4H0z" className={dir === "asc" ? "fill-s-7" : "fill-line-strong"} />
      <path d="M4 10L0 6h8z" className={dir === "desc" ? "fill-s-7" : "fill-line-strong"} />
    </svg>
  );
}
