// A six-commit history, children first as `git log --topo-order` prints it. Covered:
// - an octopus merge (f) and a two-parent merge (e);
// - a parent outside the log (b's second parent, 9…9);
// - a root (a);
// - `HEAD -> ` and `tag: ` refs;
// - a non-ASCII subject (d);
// - a subject past 120 code points whose 120th is astral (c).
export const H = (c) => c.repeat(40);
export const LONG = `${"x".repeat(119)}🚀tail`;
const LINES = [
  [H("f"), `${H("e")} ${H("d")} ${H("c")}`, "Bo", "1700000006", "tag: v1.0", "Octopus merge"],
  [H("e"), `${H("c")} ${H("d")}`, "Ana", "1700000005", "HEAD -> main, origin/main", "Merge branch 'feature'"],
  [H("d"), H("b"), "Bo", "1700000004", "feature", "Ajouté l'accès café ☕"],
  [H("c"), H("b"), "Ana", "1700000003", "", LONG],
  [H("b"), `${H("a")} ${H("9")}`, "Ana", "1700000002", "", "Second"],
  [H("a"), "", "Ana", "1700000001", "", "Initial"],
];
export const SMALL_LOG = `${LINES.map((fields) => fields.join("\x1f")).join("\n")}\n`;