import Link from "next/link";
import { Chip } from "@/components/ui/primitives";

/** A square filter toggle. Kept under this name for the listing pages. */
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
  const link = "inline-flex h-8 items-center bg-field px-3 font-mono text-[13px] text-ink hover:bg-s-2";
  return (
    <nav className="flex items-center justify-between" aria-label="Pagination">
      {page > 1 ? (
        <Link href={hrefFor(page - 1)} className={link}>
          ← Previous
        </Link>
      ) : (
        <span />
      )}
      <span className="font-mono text-[13px] text-ink-3 tabular">
        {page} / {lastPage}
      </span>
      {page < lastPage ? (
        <Link href={hrefFor(page + 1)} className={link}>
          Next →
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
    <form action={action} className="flex w-full max-w-xl" role="search">
      <label className="flex h-12 flex-1 items-center gap-3 border border-ink bg-wash px-4 focus-within:outline-2 focus-within:outline-offset-2 focus-within:outline-s-7">
        <span aria-hidden className="font-mono text-[15px] text-s-7">
          &gt;
        </span>
        <input
          type="search"
          name="q"
          defaultValue={defaultValue}
          placeholder={placeholder}
          aria-label={placeholder}
          className="h-full flex-1 bg-transparent font-mono text-[14px] text-ink outline-none placeholder:text-ink-3"
        />
      </label>
      {Object.entries(hidden).map(
        ([name, value]) => value && <input key={name} type="hidden" name={name} value={value} />,
      )}
      <button type="submit" className="h-12 bg-ink px-5 text-[15px] text-wash hover:bg-s-8">
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
