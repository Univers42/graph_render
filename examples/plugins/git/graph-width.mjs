// graph-width.mjs: how many columns the reference drawing of a history uses.
// Input is `git log --graph --format=%H` output; one graph column per commit lane, two
// characters wide, so the column of a line is `(index of "*") / 2 + 1`. The width is the
// maximum over lines, which is the number of lanes `git log --graph` ever draws at once.
//
// CLI tail: `node graph-width.mjs <file>` prints that number.
//
// Caveat: `*` also appears in a subject, but `--format=%H` leaves no subject on the line, and
// `git log --graph` puts the first `*` of a line at the lane's own column.

export function graphWidth(text) {
  let width = 0;
  const lines = text.split("\n");
  for (let i = 0; i < lines.length; i += 1) {
    const at = lines[i].indexOf("*");
    if (at < 0) continue;
    const column = Math.floor(at / 2) + 1;
    if (column > width) width = column;
  }
  return width;
}

if (import.meta.url === `file://${process.argv[1]}`) {
  const [path] = process.argv.slice(2);
  if (!path) {
    process.stderr.write("usage: graph-width.mjs <file>\n");
    process.exit(2);
  }
  const { readFileSync } = await import("node:fs");
  process.stdout.write(`${graphWidth(readFileSync(path, "utf8"))}\n`);
}