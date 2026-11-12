/** Who makes a GPU: the company whose page and logo go with a GPU type. */
export type GpuProducer = { slug: string; name: string; logo: string };

const NVIDIA: GpuProducer = { slug: "nvidia", name: "NVIDIA", logo: "/logos/nvidia.png" };
const AMD: GpuProducer = { slug: "amd", name: "AMD", logo: "/logos/amd.png" };

// NVIDIA's data-center lines (A10, A100, H100, H200, L4, L40S, T4, V100, B200, GB200, GH200)
// and its RTX, GeForce and Quadro lines; AMD's Instinct (MI3xx) and Radeon lines.
const NVIDIA_NAME = /^(?:nvidia\s+)?(?:[abhltv]\d{1,4}\b|g[hb]\d{2,3}\b|rtx|gtx|geforce|quadro|tesla|titan)/i;
const AMD_NAME = /^(?:amd\b|mi\d{3}|instinct|radeon)/i;

/**
 * The producer of a GPU type by its name (`H100 SXM`, `AMD MI300X`), or null
 * for a name no rule knows. It never guesses: a TPU or Gaudi returns null.
 */
export function gpuProducer(gpuType: string): GpuProducer | null {
  const name = gpuType.trim();
  if (AMD_NAME.test(name)) return AMD;
  if (NVIDIA_NAME.test(name)) return NVIDIA;
  return null;
}
