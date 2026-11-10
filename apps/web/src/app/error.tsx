"use client";

export default function Error({ reset }: { error: Error; reset: () => void }) {
  return (
    <div className="flex flex-col items-start gap-3">
      <h1 className="text-2xl font-semibold">Something went wrong</h1>
      <p className="text-ink-2">The data could not be loaded. Try again shortly.</p>
      <button onClick={reset} className="text-sm underline">
        Retry
      </button>
    </div>
  );
}
