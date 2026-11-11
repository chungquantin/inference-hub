#!/usr/bin/env node
/**
 * Copies the model and creator icons of architect-xyz/inference-catalog into
 * the web app, at a pinned commit:
 *
 *   node scripts/vendor-inference-catalog.mjs [path-to-a-checkout]
 *
 * Without a path it clones the repository. It copies only `logos/icons/`
 * (TypingMind's model-icons, MIT, see the license file it copies), the
 * creators and the logo rules. It does not copy `logos/providers/` or
 * `logos/harnesses/`: the catalog's NOTICE says those come from a console
 * with no license to reuse them.
 *
 * Writes:
 *   apps/web/public/catalog/icons/<id>.<ext>   the images
 *   apps/web/public/catalog/LICENSE.txt         the MIT license of the icons
 *   apps/web/src/lib/catalog.json               creators, rules, icon files
 */
import { execFileSync } from "node:child_process";
import { cpSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, parse } from "node:path";

const REPO = "https://github.com/architect-xyz/inference-catalog";
const COMMIT = "8deecc0490e9ecf4d4b18c2904d7cd498b482df3";
const root = new URL("..", import.meta.url).pathname;

let src = process.argv[2];
let temp;
if (!src) {
  temp = mkdtempSync(join(tmpdir(), "inference-catalog-"));
  execFileSync("git", ["clone", "--quiet", REPO, temp], { stdio: "inherit" });
  execFileSync("git", ["-C", temp, "checkout", "--quiet", COMMIT], { stdio: "inherit" });
  src = temp;
}
const head = execFileSync("git", ["-C", src, "rev-parse", "HEAD"]).toString().trim();
if (head !== COMMIT) {
  console.error(`expected ${COMMIT}, the checkout is at ${head}; update COMMIT on purpose`);
  process.exit(1);
}

const out = join(root, "apps/web/public/catalog");
const notice = existsSync(join(out, "NOTICE.md")) ? readFileSync(join(out, "NOTICE.md"), "utf8") : null;
rmSync(out, { recursive: true, force: true });
mkdirSync(join(out, "icons"), { recursive: true });

const icons = {};
for (const file of readdirSync(join(src, "logos/icons"))) {
  const { name, ext } = parse(file);
  if (![".jpg", ".png", ".svg"].includes(ext)) continue;
  cpSync(join(src, "logos/icons", file), join(out, "icons", file));
  icons[name] = file;
}
cpSync(join(src, "logos/icons/LICENSE.txt"), join(out, "LICENSE.txt"));
if (notice) writeFileSync(join(out, "NOTICE.md"), notice);

// Creators: id, name, aliases, and the icon id when it is an `icons/` logo.
const creators = {};
for (const file of readdirSync(join(src, "creators"))) {
  if (!file.endsWith(".json")) continue;
  const c = JSON.parse(readFileSync(join(src, "creators", file), "utf8"));
  const icon = c.logo?.startsWith("icons/") ? c.logo.slice("icons/".length) : null;
  creators[c.id] = { name: c.name, aliases: c.aliases ?? [], icon: icon && icons[icon] ? icon : null };
}

const rules = JSON.parse(readFileSync(join(src, "rules/model-logos.json"), "utf8"));
delete rules.$comment;

writeFileSync(
  join(root, "apps/web/src/lib/catalog.json"),
  `${JSON.stringify({ source: `${REPO}/tree/${COMMIT}`, icons, creators, rules }, null, 1)}\n`,
);
if (temp) rmSync(temp, { recursive: true, force: true });
console.log(`copied ${Object.keys(icons).length} icons and ${Object.keys(creators).length} creators from ${head.slice(0, 7)}`);
if (!existsSync(join(out, "LICENSE.txt"))) process.exit(1);
