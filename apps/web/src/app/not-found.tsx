import Link from "next/link";

export default function NotFound() {
  return (
    <div className="flex flex-col items-start gap-3">
      <h1 className="text-2xl font-semibold">Not found</h1>
      <p className="text-zinc-600 dark:text-zinc-400">That page is not in the hub.</p>
      <Link href="/organizations" className="text-sm underline">
        Browse the directory
      </Link>
    </div>
  );
}
