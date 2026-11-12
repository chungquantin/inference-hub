/** Paths of the pages an entity links to. */

/** Path of an organization's page. */
export function orgHref(slug: string): string {
  return `/organizations/${slug}`;
}

/** Path of a GPU type's market page. */
export function gpuHref(gpuType: string): string {
  return `/analytics/gpus/${encodeURIComponent(gpuType)}`;
}

/** Path of a model's market page, by its normalized key. */
export function modelHref(key: string): string {
  return `/analytics/models/${encodeURIComponent(key)}`;
}
