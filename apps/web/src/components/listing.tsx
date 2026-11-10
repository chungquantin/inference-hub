import Link from "next/link";
import { Chip } from "@/components/ui/primitives";

/** A filter pill. Kept under this name for the listing pages. */
export function FilterChip(props: { href: string; active: boolean; children: React.ReactNode }) {
  return <Chip {...props} />;
}

/** Previous / next links for a paged listing. */
export function Pagination({
  page,
  lastPage,
  hrefFor,
}: {
  page: number;
  lastPage: number;
  hrefFor: (page: number) => string;
}) {
  if (lastPage <= 1) return null;
  const link = "rounded-[6px] px-3 py-1.5 text-ink-2 hover:bg-hover hover:text-ink";
  return (
    <nav className="flex items-center justify-between text-sm" aria-label="Pagination">
      {page > 1 ? (
        <Link href={hrefFor(page - 1)} className={link}>
          Previous
        </Link>
      ) : (
        <span />
      )}
      <span className="text-ink-3">
        Page {page} of {lastPage}
      </span>
      {page < lastPage ? (
        <Link href={hrefFor(page + 1)} className={link}>
          Next
        </Link>
      ) : (
        <span />
      )}
    </nav>
  );
}

/** A search box that submits as a GET form, keeping the other filters as hidden fields. */
export function SearchForm({
  action,
  defaultValue,
  placeholder,
  hidden = {},
}: {
  action?: string;
  defaultValue?: string;
  placeholder: string;
  hidden?: Record<string, string | undefined>;
}) {
  return (
    <form action={action} className="flex w-full max-w-xl gap-2" role="search">
      <label className="flex h-10 flex-1 items-center gap-2 rounded-[6px] bg-surface px-3 shadow-elev-1 focus-within:outline-2 focus-within:outline-accent">
        <svg width="16" height="16" viewBox="0 0 16 16" fill="none" aria-hidden className="shrink-0 text-ink-3">
          <circle cx="7" cy="7" r="4.5" stroke="currentColor" strokeWidth="1.5" />
          <path d="m10.5 10.5 3 3" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
        </svg>
        <input
          type="search"
          name="q"
          defaultValue={defaultValue}
          placeholder={placeholder}
          aria-label={placeholder}
          className="h-full flex-1 bg-transparent text-sm text-ink outline-none placeholder:text-ink-3"
        />
      </label>
      {Object.entries(hidden).map(
        ([name, value]) => value && <input key={name} type="hidden" name={name} value={value} />,
      )}
      <button
        type="submit"
        className="h-10 rounded-[6px] bg-ink px-4 text-sm font-medium text-canvas hover:opacity-90"
      >
        Search
      </button>
    </form>
  );
}

/** Builds a listing URL from the current filters plus overrides, dropping empty values. */
export function listingHref(
  path: string,
  current: Record<string, string | undefined>,
  overrides: Record<string, string | undefined> = {},
): string {
  const params = new URLSearchParams();
  for (const [key, value] of Object.entries({ ...current, ...overrides })) {
    if (value) params.set(key, value);
  }
  const qs = params.toString();
  return qs ? `${path}?${qs}` : path;
}

/** Parses a 1-based page number from the query string. */
export function parsePage(value: string | undefined): number {
  return Math.max(1, Number.parseInt(value ?? "1", 10) || 1);
}
