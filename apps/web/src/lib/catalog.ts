import catalog from "@/lib/catalog.json";

/**
 * Model and creator logos from architect-xyz/inference-catalog (icons by
 * TypingMind's model-icons, MIT; see public/catalog/LICENSE.txt). The files
 * are copied by `scripts/vendor-inference-catalog.mjs` at a pinned commit.
 *
 * The resolution follows the catalog's rules: exact model name, then the
 * family (first word of the name), then the creator's icon, then the
 * creator id as an icon, then Hugging Face for a known creator.
 */

type Creator = { name: string; aliases: string[]; icon: string | null };

const ICONS = catalog.icons as Record<string, string>;
const CREATORS = catalog.creators as Record<string, Creator>;
const RULES = catalog.rules as {
  exact: Record<string, string>;
  families: Record<string, string>;
  familyAliases: Record<string, string>;
  fallback: string;
};

const BASE = "/catalog/icons";

// Creator lookup by id, alias and name, in lower case.
const byKey = new Map<string, Creator & { id: string }>();
for (const [id, c] of Object.entries(CREATORS)) {
  const entry = { id, ...c };
  for (const key of [id, ...c.aliases, c.name]) byKey.set(key.trim().toLowerCase(), entry);
}

const url = (icon: string | null | undefined): string | null =>
  icon && ICONS[icon] ? `${BASE}/${ICONS[icon]}` : null;

/** The creator of the catalog for an id, alias or name (`meta-llama` → Meta), or undefined. */
export function catalogCreator(key: string | null | undefined) {
  return key ? byKey.get(key.trim().toLowerCase()) : undefined;
}

/** The logo of a model maker, such as `deepseek-ai` or `meta-llama`, or null. */
export function creatorLogoUrl(maker: string | null | undefined): string | null {
  if (!maker) return null;
  const creator = catalogCreator(maker);
  return url(creator?.icon) ?? url(maker.trim().toLowerCase());
}

/** The display name of a model maker: the catalog's name, or the maker id as given. */
export function creatorName(maker: string | null | undefined): string | null {
  if (!maker) return null;
  return catalogCreator(maker)?.name ?? maker;
}

function family(name: string): string | null {
  const word = /^[a-z]+/.exec(name)?.[0];
  return word ? (RULES.familyAliases[word] ?? word) : null;
}

/**
 * The logo of a model, from its id or market key and its maker. Returns null
 * when nothing is known, so the caller draws a monogram.
 */
export function modelLogoUrl(model: string, maker?: string | null): string | null {
  const full = model.trim().toLowerCase();
  const slash = full.lastIndexOf("/");
  const name = slash === -1 ? full : full.slice(slash + 1);
  const prefix = slash === -1 ? undefined : full.slice(0, full.indexOf("/"));
  const key = maker?.trim().toLowerCase() || prefix;
  const fam = family(name);
  const creator = key ? catalogCreator(key) : undefined;
  for (const icon of [
    RULES.exact[name],
    fam ? RULES.families[fam] : undefined,
    // A family word that is itself an icon: `gemini`, `qwen`, `mistral`.
    fam,
    creator?.icon,
    key,
    creator ? RULES.fallback : undefined,
  ]) {
    const found = url(icon);
    if (found) return found;
  }
  return null;
}
