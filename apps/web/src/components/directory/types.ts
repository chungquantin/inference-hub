export type Tab = "gpus" | "models" | "orgs";

/** One row of the directory, whichever tab it belongs to. Fields a tab does not use are left out. */
export type Row = {
  id: string;
  name: string;
  /** A second line under the name: the maker, a tagline or the cloud count. */
  sub: string;
  /** The row's detail page. */
  href: string;
  logo?: string | null;
  /** Lowest, median and highest price. GPUs are per hour, models per 1M tokens. */
  lo?: number;
  mid?: number;
  hi?: number;
  /** Highest over lowest (GPUs), or median over lowest (models). */
  gap?: number;
  clouds?: number;
  offers?: number;
  providers?: number;
  at?: string;
  /** The cheapest seller as an entity, for a link with its logo. */
  atOrg?: { slug: string; name: string; logo: string | null };
  /** The company that makes a GPU, by name (`NVIDIA`): for sorting and search. */
  producer?: string;
  /** The maker of a model, as the market lists it (`deepseek-ai`). */
  maker?: string;
  ctx?: number | null;
  caps?: string[];
  inPrice?: number;
  outPrice?: number;
  kinds?: string[];
  tagline?: string;
  web?: string;
  country?: string;
  founded?: number | null;
  status?: string;
};

/**
 * `models` starts as the comparable models (sold by 2+ providers) so the tab is
 * instant; the full list is fetched on demand. `modelTotal` is every model.
 */
export type DirectoryData = { gpus: Row[]; models: Row[]; orgs: Row[]; modelTotal: number };

/** The compact payload of `/api/directory/models`: one array per model, organizations once. */
export type ModelsPayload = {
  /** Cheapest sellers, referenced by index: [slug, name, logo]. */
  orgs: [string, string, string | null][];
  /** [key, maker, providers, lowest, median, input, output, context, capabilities, cheapest org index] */
  rows: [string, string | null, number, number, number, number, number, number | null, string, number][];
  total: number;
};
