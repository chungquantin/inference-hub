import { formatDate } from "@/lib/format";

/** When a price was last checked, linking to its source when there is one. */
export function CheckedOn({ date, source }: { date: string; source?: string | null }) {
  const label = formatDate(date);
  return source ? (
    <a href={source} rel="noopener" target="_blank" className="text-ink-3 underline hover:text-s-7">
      {label}
    </a>
  ) : (
    <span className="text-ink-3">{label}</span>
  );
}
