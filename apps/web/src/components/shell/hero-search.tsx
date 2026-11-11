"use client";

import { useRouter } from "next/navigation";
import { useState } from "react";
import { openCommandPalette } from "@/components/shell/command-palette";
import { SearchIcon } from "@/components/shell/main-nav";

/** Quick starts: what people most often look up. */
const EXAMPLES = ["DeepSeek V3", "H100", "Claude Sonnet", "Whisper", "OpenRouter"];

/**
 * The home page's search: one large field that submits to /search, with
 * example searches under it. With JavaScript, ⌘K opens live results; without,
 * it is a plain form.
 */
export function HeroSearch({ placeholder }: { placeholder: string }) {
  const router = useRouter();
  const [q, setQ] = useState("");
  return (
    <div className="flex flex-col gap-3">
      <form
        action="/search"
        role="search"
        onSubmit={(e) => {
          e.preventDefault();
          if (q.trim()) router.push(`/search?q=${encodeURIComponent(q.trim())}`);
          else openCommandPalette();
        }}
        className="flex h-14 border border-line-strong bg-wash focus-within:border-[color:var(--s-7)] focus-within:outline-2 focus-within:outline-offset-2 focus-within:outline-s-7"
      >
        <label className="flex flex-1 items-center gap-3 px-4 text-ink-3">
          <SearchIcon />
          <input
            type="search"
            name="q"
            value={q}
            onChange={(e) => setQ(e.target.value)}
            placeholder={placeholder}
            aria-label="Search models, GPUs and companies"
            className="h-full min-w-0 flex-1 bg-transparent text-[16px] text-ink outline-none placeholder:text-ink-3"
          />
          <kbd className="hidden border border-line px-1.5 font-mono text-[11px] text-ink-3 sm:inline">⌘K</kbd>
        </label>
        <button type="submit" className="bg-s-7 px-6 text-[15px] font-medium text-wash hover:bg-s-8">
          Search
        </button>
      </form>
      <p className="flex flex-wrap items-center gap-2 text-[14px] text-ink-3">
        Popular:
        {EXAMPLES.map((ex) => (
          <a
            key={ex}
            href={`/search?q=${encodeURIComponent(ex)}`}
            className="bg-wash px-2.5 py-0.5 text-ink-2 shadow-[inset_0_0_0_1px_var(--line)] hover:text-s-7 hover:shadow-[inset_0_0_0_1px_var(--s-7)]"
          >
            {ex}
          </a>
        ))}
      </p>
    </div>
  );
}
