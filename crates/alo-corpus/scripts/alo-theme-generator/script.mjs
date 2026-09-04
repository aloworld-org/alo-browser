// Generates the Tailwind theme from the design tokens (ADR 0046).
//
// ADR 0045's strongest objection to Tailwind was that adopting it would mean
// losing the token layer — two sources of truth for a colour, drifting apart.
// This script is the answer: `src/ds/tokens.css` stays the single definition,
// and the theme is derived from it, so `--bg-surface` and `bg-surface` are the
// same value spelled two ways and cannot disagree.
//
// Run:    node scripts/gen-tailwind-theme.mjs           # write the theme
//         node scripts/gen-tailwind-theme.mjs --check   # fail if out of date
//
// The --check form is the tripwire ADR 0046 names: if this ever fails in CI or
// a gate, the generated theme has drifted from the tokens and the decision to
// adopt Tailwind has lost the property it was justified by.
//
// ---------------------------------------------------------------------------
// Why `@theme inline reference`, and why it is not a detail (D1.51)
//
// A plain `@theme` block *emits* its variables. Four of alo's token families
// are spelled exactly like the Tailwind namespace they belong to —
// `--radius-*`, `--shadow-*`, `--font-*` and the `--text-*` type scale — so the
// derived theme said `--radius-sm: var(--radius-sm)`, a custom property that
// references itself. A self-referential property is invalid at computed-value
// time: it does not fall back to the earlier declaration, it resolves to
// nothing.
//
// It never fired, and the reason it never fired is luck rather than design:
// Tailwind emits its theme inside `@layer theme`, and an unlayered declaration
// beats any layer, so tokens.css — which is in no layer — kept winning. The day
// anybody wraps tokens.css in a layer of its own, or Tailwind changes where it
// puts the theme, every radius, shadow and font-family in the product resolves
// to nothing at once, and the source looks correct on both sides.
//
// `inline` makes each utility read the token directly
// (`.rounded-md { border-radius: var(--radius-md) }`) and `reference` stops the
// block emitting variables at all, so the token keeps being the only
// declaration of its own name and the collision cannot come back. Never drop
// either keyword without renaming the four families first.
// ---------------------------------------------------------------------------

import { readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const here = dirname(fileURLToPath(import.meta.url));
const TOKENS = join(here, "..", "src", "ds", "tokens.css");
const OUT = join(here, "..", "src", "ds", "theme.css");

// Only the SEMANTIC tokens become utilities. The raw scale (--verdigris-300,
// --navy) stays private on purpose: tokens.css says components must use the
// semantic layer and never ad-hoc values, and exposing the scale as utilities
// would hand every screen a way around that rule.
const SEMANTIC =
  /^(bg|text|border|accent|shadow|radius|space|font|ease|weight|leading|danger|success|warning|unread|control-height)/;

// A token's value tells us what it is when its name cannot. `--text-primary`
// and `--text-sm` share a prefix and are not the same kind of thing: one is a
// colour, the other a font size, and Tailwind puts them in different
// namespaces. Length ⇒ size.
const LENGTH = /^-?[\d.]+(rem|px|em)$/;

// Tokens whose Tailwind name is not derivable from their prefix.
const EXPLICIT = new Map([
  // The control heights are `h-control` / `h-control-lg`, because a form
  // control's height is a decision of this design system and not a step on the
  // spacing scale that happens to match.
  ["control-height", ["spacing", "control"]],
  ["control-height-lg", ["spacing", "control-lg"]],
]);

// Prefix → Tailwind namespace, for the tokens whose kind their name does give
// away. Longest prefix first: `font-size-` would otherwise be eaten by `font-`.
const NAMESPACE = [
  ["bg-", "color"],
  ["border-", "color"],
  ["shadow-", "shadow"],
  ["radius-", "radius"],
  ["space-", "spacing"],
  ["weight-", "font-weight"],
  ["leading-", "leading"],
  ["ease-", "ease"],
  ["font-", "font"],
];

const source = readFileSync(TOKENS, "utf8");
// Values wrap across lines (`--font-ui`, the gradients), so the file is read as
// declarations rather than as lines. Comments go first: they sit after the
// semicolon they follow and would otherwise ride along in the next value.
const declarations = source
  .replace(/\/\*[\s\S]*?\*\//g, "")
  .split(";")
  .map((chunk) => chunk.match(/--([a-z0-9-]+)\s*:\s*([\s\S]+)$/))
  .filter((m) => m !== null)
  .map((m) => [m[1], m[2].trim().replace(/\s+/g, " ")]);

/** The `@theme` key a token maps onto, or null if it is not part of the
 *  public surface. Returns `[namespace, name]`. */
function target(name, value) {
  const explicit = EXPLICIT.get(name);
  if (explicit !== undefined) return explicit;
  if (!SEMANTIC.test(name)) return null;
  // The accent family keeps its own name whole: `accent-hover` is one word for
  // one role, and slicing the prefix off it leaves `--color--hover`.
  if (name === "accent" || name.startsWith("accent-")) return ["color", name];
  if (/^(danger|success|warning|unread)/.test(name)) return ["color", name];
  if (name.startsWith("text-")) {
    // The type scale, which Tailwind keeps in its own `--text-*` namespace.
    return LENGTH.test(value)
      ? ["text", name.slice("text-".length)]
      : ["color", name.slice("text-".length)];
  }
  if (name.startsWith("border-") && LENGTH.test(value)) {
    // `--border-width` is geometry, not colour, and Tailwind has no theme
    // namespace for it — border widths are the static `border` / `border-2`.
    return null;
  }
  const hit = NAMESPACE.find(([p]) => name.startsWith(p));
  if (hit === undefined) return null;
  return [hit[1], name.slice(hit[0].length)];
}

const lines = [];
const seen = new Map();
let count = 0;
for (const [name, value] of declarations) {
  const mapped = target(name, value);
  if (mapped === null) continue;
  count += 1;
  const key = `--${mapped[0]}-${mapped[1]}`;
  const clash = seen.get(key);
  if (clash !== undefined) {
    console.error(
      `two tokens map onto ${key}: --${clash} and --${name}. ` +
        `One utility cannot mean two things — rename a token or teach EXPLICIT.`,
    );
    process.exit(1);
  }
  seen.set(key, name);
  lines.push(`  ${key}: var(--${name});`);
}
if (lines.length === 0) {
  console.error(
    "no semantic tokens found in tokens.css — refusing to write an empty theme",
  );
  process.exit(1);
}

const generated = `/* GENERATED by scripts/gen-tailwind-theme.mjs — do not edit.
 *
 * Every value here is a reference to a token in tokens.css, never a literal.
 * That is the whole point (ADR 0046): one definition, two spellings. To change
 * a colour, change tokens.css and re-run the generator.
 *
 * ${lines.length} utilities derived from ${count} semantic tokens.
 */
@import "tailwindcss";

/* \`inline reference\`, not a bare \`@theme\`: see the header of the generator.
 * Four token families are spelled like the namespace they belong to, so an
 * emitted \`--radius-sm: var(--radius-sm)\` would be a property that resolves to
 * nothing. \`inline\` points each utility at the token; \`reference\` stops this
 * block emitting any variable of its own. */
@theme inline reference {
  /* Tailwind's own type scale is cleared rather than partially overridden. Its
   * sizes ship paired line-heights (\`--text-base--line-height\`), and those
   * survive a redefinition of the size alone — so \`text-base\` would quietly
   * set a line-height no stylesheet here asked for. */
  --text-*: initial;

  /* And its colour palette, for the reason the generator's own comment gives
   * for keeping the raw scale private: a rule nothing checks is a rule that
   * drifts. Until D1.55 cleared this, \`bg-red-500\` and \`text-slate-400\`
   * compiled happily — 22 families of them — so "semantic utilities only" was
   * a convention, and every hard-coded colour this design system spent a wave
   * removing had a shorter way back in than the token it replaced. Now it does
   * not compile, which is the same mechanism as \`primitives.test.ts\`: the
   * build holds the line, not the reviewer.
   *
   * \`transparent\`, \`current\` and \`inherit\` are not theme values in Tailwind
   * v4 — they are built into the utilities themselves — so \`bg-transparent\`
   * and \`border-transparent\` survive this and are still the right way to say
   * "no colour here". */
  --color-*: initial;

${lines.join("\n")}
}
`;

if (process.argv.includes("--check")) {
  let current = "";
  try {
    current = readFileSync(OUT, "utf8");
  } catch {
    /* missing counts as out of date */
  }
  if (current !== generated) {
    console.error(
      "theme.css is out of date with tokens.css — run: node scripts/gen-tailwind-theme.mjs",
    );
    process.exit(1);
  }
  console.log(`theme.css is current (${lines.length} utilities)`);
} else {
  writeFileSync(OUT, generated);
  console.log(
    `wrote ${OUT} — ${lines.length} utilities from ${count} semantic tokens`,
  );
}
