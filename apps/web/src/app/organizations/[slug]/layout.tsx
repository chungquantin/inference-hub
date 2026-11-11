export default function WideLayout({ children }: { children: React.ReactNode }) {
  // Marks the page as wide; see `body:has([data-wide])` in globals.css.
  return <div data-wide>{children}</div>;
}
