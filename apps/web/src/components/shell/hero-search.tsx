"use client";

import { useRouter } from "next/navigation";
import { useState } from "react";
import { openCommandPalette } from "@/components/shell/command-palette";

const EXAMPLES = ["deepseek v3", "h100", "claude sonnet", "whisper", "openrouter"];

/**
 * The home page's search: a big field that submits to /search, with example
 * queries. Typing into it with JavaScript opens the command palette's live
 * results; without JavaScript it is a plain form.
 */
export function HeroSearch() {
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
        className="flex h-14 border border-ink bg-wash focus-within:outline-2 focus-within:outline-offset-2 focus-within:outline-s-7"
      >
        <label className="flex flex-1 items-center gap-3 px-4">
          <span aria-hidden className="font-mono text-[16px] text-s-7">
            &gt;
          </span>
          <input
            type="search"
            name="q"
            value={q}
            onChange={(e) => setQ(e.target.value)}
            placeholder="Search 22,000 models, 60 GPU types, 50 providers"
            aria-label="Search the hub"
            className="h-full flex-1 bg-transparent font-mono text-[15px] text-ink outline-none placeholder:text-ink-3"
          />
          <kbd className="hidden border border-line px-1.5 font-mono text-[11px] text-ink-3 sm:inline">⌘K</kbd>
        </label>
        <button type="submit" className="bg-ink px-6 text-[15px] text-wash hover:bg-s-8">
          Search
        </button>
      </form>
      <p className="flex flex-wrap items-center gap-2 font-mono text-[12px] text-ink-3">
        Try
        {EXAMPLES.map((ex) => (
          <a
            key={ex}
            href={`/search?q=${encodeURIComponent(ex)}`}
            className="bg-field px-2 py-0.5 text-ink-2 hover:bg-s-2 hover:text-ink"
          >
            {ex}
          </a>
        ))}
      </p>
    </div>
  );
}
