import Image from "next/image";

const SIZES = { xs: 16, sm: 20, md: 28, lg: 56 } as const;

/**
 * An organization's logo on a square white plate with a hairline frame. Without
 * a logo, the plate shows the name's first letter on a brand-ramp color picked
 * from the name, so the same company always gets the same color.
 */
export function Logo({
  name,
  src,
  size = "md",
}: {
  name: string;
  src?: string | null;
  size?: keyof typeof SIZES;
}) {
  const px = SIZES[size];
  if (src) {
    return (
      <span
        className="inline-flex shrink-0 items-center justify-center overflow-hidden border border-line bg-wash"
        style={{ width: px, height: px }}
      >
        <Image src={src} alt="" width={px} height={px} className="h-full w-full object-contain p-[10%]" />
      </span>
    );
  }
  const shades = ["#0b6e5f", "#08584c", "#2a8a75", "#064239", "#57ab96"];
  return (
    <span
      aria-hidden
      className="inline-flex shrink-0 items-center justify-center font-mono font-medium text-wash"
      style={{
        width: px,
        height: px,
        fontSize: Math.round(px * 0.45),
        background: shades[hash(name) % shades.length],
      }}
    >
      {name.trim().charAt(0).toUpperCase()}
    </span>
  );
}

function hash(name: string): number {
  let h = 0;
  for (const c of name) h = (h * 31 + c.charCodeAt(0)) >>> 0;
  return h;
}
