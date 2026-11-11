/** A placeholder block while a page loads; announces itself to screen readers once. */
export function PageSkeleton({ rows = 10, filters = true }: { rows?: number; filters?: boolean }) {
  return (
    <div className="flex flex-col gap-6" role="status" aria-busy="true" aria-label="Loading">
      <div className="flex flex-col gap-4 border-b border-line pb-8">
        <div className="h-4 w-40 animate-pulse bg-field" />
        <div className="h-10 w-80 max-w-full animate-pulse bg-field" />
        <div className="h-5 w-[560px] max-w-full animate-pulse bg-field" />
      </div>
      {filters && <div className="h-[62px] animate-pulse border border-line bg-paper" />}
      <div className="border border-line">
        <div className="h-10 border-b border-line bg-paper" />
        {Array.from({ length: rows }, (_, i) => (
          <div key={i} className="flex h-11 items-center gap-4 border-b border-line px-3 last:border-0">
            <div className="h-3 w-48 animate-pulse bg-field" />
            <div className="ml-auto h-3 w-16 animate-pulse bg-field" />
            <div className="h-3 w-16 animate-pulse bg-field" />
          </div>
        ))}
      </div>
    </div>
  );
}
