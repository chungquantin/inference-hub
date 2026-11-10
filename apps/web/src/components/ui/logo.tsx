import Image from "next/image";

const SIZES = { sm: 20, md: 28, lg: 48 } as const;

/**
 * An organization's logo on a white plate, so any brand mark reads on both
 * themes. Without a logo, the plate shows the name's first letter on a color
 * derived from the name, so the same company always gets the same color.
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
  const radius = size === "lg" ? "rounded-[10px]" : "rounded-[6px]";
  if (src) {
    return (
      <span
        className={`inline-flex shrink-0 items-center justify-center overflow-hidden bg-logo-plate shadow-logo ${radius}`}
        style={{ width: px, height: px }}
      >
        <Image
          src={src}
          alt=""
          width={px}
          height={px}
          className="h-full w-full object-contain p-[12%]"
          unoptimized={src.endsWith(".svg")}
        />
      </span>
    );
  }
  return (
    <span
      aria-hidden
      className={`inline-flex shrink-0 items-center justify-center font-display font-semibold text-white ${radius}`}
      style={{
        width: px,
        height: px,
        fontSize: Math.round(px * 0.46),
        background: `hsl(${hue(name)} 42% 42%)`,
      }}
    >
      {name.trim().charAt(0).toUpperCase()}
    </span>
  );
}

function hue(name: string): number {
  let h = 0;
  for (const c of name) h = (h * 31 + c.charCodeAt(0)) % 360;
  return h;
}
