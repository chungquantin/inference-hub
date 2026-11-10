import Link from "next/link";

/** A pill link used for filters. */
export function FilterChip({
  href,
  active,
  children,
}: {
  href: string;
  active: boolean;
  children: React.ReactNode;
}) {
  return (
    <Link
      href={href}
      aria-current={active ? "page" : undefined}
      className={
        active
          ? "rounded-full bg-zinc-900 px-3 py-1 text-white dark:bg-zinc-100 dark:text-zinc-900"
          : "rounded-full border border-zinc-300 px-3 py-1 text-zinc-700 hover:border-zinc-500 dark:border-zinc-700 dark:text-zinc-300"
      }
    >
      {children}
    </Link>
  );
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
  return (
    <nav className="flex items-center justify-between text-sm" aria-label="Pagination">
      {page > 1 ? (
        <Link href={hrefFor(page - 1)} className="hover:underline">
          ← Previous
        </Link>
      ) : (
        <span />
      )}
      <span className="text-zinc-500">
        Page {page} of {lastPage}
      </span>
      {page < lastPage ? (
        <Link href={hrefFor(page + 1)} className="hover:underline">
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
    <form action={action} className="flex max-w-xl gap-2" role="search">
      <input
        type="search"
        name="q"
        defaultValue={defaultValue}
        placeholder={placeholder}
        aria-label={placeholder}
        className="flex-1 rounded-md border border-zinc-300 bg-transparent px-3 py-2 text-sm outline-none focus:border-zinc-500 dark:border-zinc-700"
      />
      {Object.entries(hidden).map(
        ([name, value]) => value && <input key={name} type="hidden" name={name} value={value} />,
      )}
      <button
        type="submit"
        className="rounded-md bg-zinc-900 px-4 py-2 text-sm font-medium text-white dark:bg-zinc-100 dark:text-zinc-900"
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
