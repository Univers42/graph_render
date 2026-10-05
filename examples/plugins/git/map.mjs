// `git log` text (FORMAT) to the rows the SDK's rows adapter maps onto the ingest contract.
// Pure: no I/O, so the tests run it on a string and the bench times it alone.

export const FORMAT = "%H%x1f%P%x1f%an%x1f%at%x1f%D%x1f%s";
export const SUBJECT_MAX = 120;
const HASH = /^(?:[0-9a-f]{40}|[0-9a-f]{64})$/;
const SECONDS = /^\d{1,10}$/;
const U32_MAX = 0xffff_ffff;

// `refs` is a scalar column: tag hubs would add a lane per distinct ref to a history drawing.
// `decorate.mjs` copies it onto node `tags` for the studio's `tag:#x` query instead.
export const COLUMNS = [
  { name: "subject", role: "title" },
  { name: "author", role: "group" },
  { name: "parents", label: "parent", role: "link", link: { collection: "commit", cardinality: "many", symmetric: false } },
  { name: "refs", role: "scalar" },
];

export class LogRefusal extends Error {
  constructor(line, why) {
    super(`line ${line}: ${why}`);
    this.name = "LogRefusal";
    this.line = line;
  }
}

/** Every commit, in log order. A record without six fields, a hash that is not 40 or 64 hex
 *  digits, a time that is not a u32, or a commit seen twice is refused with its line. */
export function parseLog(text) {
  const commits = [];
  const seen = new Set();
  const lines = text.split("\n");
  for (let i = 0; i < lines.length; i += 1) {
    if (lines[i] === "") continue;
    const commit = parseLine(lines[i], i + 1);
    if (seen.has(commit.hash)) throw new LogRefusal(i + 1, `commit ${commit.hash} appears twice`);
    seen.add(commit.hash);
    commits.push(commit);
  }
  return commits;
}

function parseLine(line, at) {
  const fields = line.split("\x1f");
  if (fields.length !== 6) throw new LogRefusal(at, `expected 6 fields separated by \\x1f, found ${fields.length}`);
  const [hash, parentText, author, time, decoration, subject] = fields;
  const parents = parentText === "" ? [] : parentText.split(" ");
  for (const id of [hash, ...parents]) {
    if (!HASH.test(id)) throw new LogRefusal(at, `not a 40 or 64 digit hex hash: ${JSON.stringify(id)}`);
  }
  if (!SECONDS.test(time) || Number(time) > U32_MAX) {
    throw new LogRefusal(at, `time is not a u32 count of seconds: ${JSON.stringify(time)}`);
  }
  return {
    hash, parents, author: author === "" ? undefined : author, time: Number(time),
    refs: refsOf(decoration, parents.length), subject: cut(subject),
  };
}

/** `%D` as tag values, `HEAD -> ` and `tag: ` stripped, then `merge` for two or more parents
 *  and `root` for none, counted on `%P` as git printed it (before missing parents drop).
 *  Caveat: a ref literally named `merge` or `root` is indistinguishable from the synthetic
 *  tag; the set keeps one. */
export function refsOf(decoration, parentCount) {
  const refs = decoration === "" ? [] : decoration.split(", ").map(strip);
  if (parentCount >= 2) refs.push("merge");
  if (parentCount === 0) refs.push("root");
  return [...new Set(refs)];
}

function strip(ref) {
  if (ref.startsWith("HEAD -> ")) return ref.slice("HEAD -> ".length);
  if (ref.startsWith("tag: ")) return ref.slice("tag: ".length);
  return ref;
}

// Array.from walks code points, so a surrogate pair is never split. An empty subject is an
// absent cell: the motor then labels the node with its record id.
function cut(subject) {
  if (subject === "") return undefined;
  const points = Array.from(subject);
  return points.length <= SUBJECT_MAX ? subject : points.slice(0, SUBJECT_MAX).join("");
}

/** The rows source named `name`. A parent absent from the log is dropped, because the
 *  contract refuses a dangling link; `dropped` counts them so the caller can say so. */
export function toRows(commits, name) {
  const known = new Set(commits.map((c) => c.hash));
  let dropped = 0;
  const rows = commits.map((c) => {
    const parents = c.parents.filter((p) => known.has(p));
    dropped += c.parents.length - parents.length;
    return { id: c.hash, updatedAt: c.time, values: { subject: c.subject, author: c.author, parents, refs: c.refs } };
  });
  const table = { id: "commit", name: "Commit", titleColumn: "subject", columns: COLUMNS, rows };
  return { rows: { source: name, tables: [table] }, dropped };
}