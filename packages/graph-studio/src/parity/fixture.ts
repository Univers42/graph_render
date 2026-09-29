/**
 * `fixtures/scigraphs/lesmis.json`, read into the shape the parity page draws. The file is
 * SciGraphs' own output — its camera projection, its RANK coordinate, its label set — so
 * every member is checked for the shape the painter reads and refused when it is not,
 * rather than handed on as `any` and discovered as a NaN on the canvas.
 *   docs/decisions/scigraphs-reference-fixture.md
 *   harness/scigraphs_lesmis_fixture.py (the generator, the four label stages)
 */

/** Where the studio stages it, and what the parity page fetches. */
export const FIXTURE_PATH = "fixtures/scigraphs/lesmis.json";

export interface FixtureNode {
  readonly id: number;
  readonly label: string;
  readonly betweenness: number;
  /** The RANK colour coordinate (colormaps.py:527-530). */
  readonly t: number;
  readonly world: readonly [number, number, number];
  /** SciGraphs' own pixel position in the 1920x1080 fig1 frame. */
  readonly screen: readonly [number, number];
  /** The node's distance along the view axis, from the camera to it. */
  readonly depth: number;
}

export interface FixtureParams {
  readonly resolution: readonly [number, number];
  readonly camera_lens_mm: number;
  readonly camera_sensor_mm: number;
  readonly camera_margin: number;
  /** How far behind the centre of the cloud the camera sits (executor.py:652-744). */
  readonly camera_distance: number;
  /** The unit the camera looks along, and the world axis the box is measured on. */
  readonly camera_forward: readonly [number, number, number];
  readonly bbox_lo: readonly [number, number, number];
  readonly bbox_hi: readonly [number, number, number];
  readonly node_radius_rel: number;
  readonly edge_radius_rel: number;
  readonly label_font_size: number;
  readonly label_max_count: number;
  readonly color_norm: string;
}

export interface Fixture {
  readonly nodes: readonly FixtureNode[];
  readonly edges: readonly (readonly [number, number])[];
  /** The node indices the reference pipeline kept, in its own order. */
  readonly labels: readonly number[];
  readonly params: FixtureParams;
  readonly radii: { readonly R: number; readonly node: number; readonly edge: number };
}

function refuse(what: string, detail: string): never {
  throw new Error(`the scigraphs fixture's ${what} is not the shape the parity page draws: ${detail}`);
}

function record(value: unknown, what: string): Record<string, unknown> {
  if (typeof value !== "object" || value === null || Array.isArray(value)) refuse(what, "not an object");
  return { ...value };
}

function list(value: unknown, what: string): unknown[] {
  if (!Array.isArray(value)) refuse(what, "not an array");
  return value;
}

function num(value: unknown, what: string): number {
  if (typeof value !== "number" || !Number.isFinite(value)) refuse(what, `${String(value)} is not a finite number`);
  return value;
}

function index(value: unknown, what: string, count: number): number {
  const at = num(value, what);
  if (!Number.isInteger(at) || at < 0 || at >= count) refuse(what, `${at} is not a node index in 0..${count - 1}`);
  return at;
}

/** `count` finite numbers in a fixed-length array, as a tuple the painter can read. */
function measured(value: unknown, what: string, count: number): readonly number[] {
  const items = list(value, what);
  if (items.length !== count) refuse(what, `${items.length} values, not ${count}`);
  return items.map((item, at) => num(item, `${what}[${at}]`));
}

function pair(value: unknown, what: string): readonly [number, number] {
  const [first, second] = measured(value, what, 2);
  return [first ?? 0, second ?? 0];
}

function triple(value: unknown, what: string): readonly [number, number, number] {
  const [first, second, third] = measured(value, what, 3);
  return [first ?? 0, second ?? 0, third ?? 0];
}

function nodeOf(value: unknown, at: number): FixtureNode {
  const what = `node ${at}`;
  const node = record(value, what);
  const id = num(node.id, `${what}.id`);
  if (id !== at) refuse(what, `id ${id} is not its own position`);
  const t = num(node.t, `${what}.t`);
  if (t < 0 || t > 1) refuse(`${what}.t`, `${t} is outside 0..1`);
  const depth = num(node.depth, `${what}.depth`);
  if (depth <= 0) refuse(`${what}.depth`, `${depth} is not in front of the camera`);
  return {
    id, t, depth,
    label: typeof node.label === "string" ? node.label : refuse(`${what}.label`, "not a string"),
    betweenness: num(node.betweenness, `${what}.betweenness`),
    world: triple(node.world, `${what}.world`),
    screen: pair(node.screen, `${what}.screen`),
  };
}

function nodesOf(value: unknown): FixtureNode[] {
  const items = list(value, "nodes");
  if (items.length === 0) refuse("nodes", "the graph is empty");
  return items.map(nodeOf);
}

function edgesOf(value: unknown, count: number): (readonly [number, number])[] {
  return list(value, "edges").map((item, at) => {
    const ends = list(item, `edge ${at}`);
    if (ends.length !== 2) refuse(`edge ${at}`, `${ends.length} ends, not 2`);
    const [from, to] = [index(ends[0], `edge ${at} source`, count), index(ends[1], `edge ${at} target`, count)];
    return [from, to] as const;
  });
}

function labelsOf(value: unknown, count: number): number[] {
  const kept = list(value, "labels").map((item, at) => index(item, `label ${at}`, count));
  if (new Set(kept).size !== kept.length) refuse("labels", "a node is named twice");
  return kept;
}

function paramsOf(value: unknown): FixtureParams {
  const params = record(value, "params");
  const positive = (name: string): number => {
    const read = num(params[name], `params.${name}`);
    if (!(read > 0)) refuse(`params.${name}`, `${read} is not positive`);
    return read;
  };
  return {
    resolution: pair(params.resolution, "params.resolution"),
    camera_lens_mm: positive("camera_lens_mm"),
    camera_sensor_mm: positive("camera_sensor_mm"),
    camera_margin: positive("camera_margin"),
    camera_distance: positive("camera_distance"),
    camera_forward: triple(params.camera_forward, "params.camera_forward"),
    bbox_lo: triple(params.bbox_lo, "params.bbox_lo"),
    bbox_hi: triple(params.bbox_hi, "params.bbox_hi"),
    node_radius_rel: positive("node_radius_rel"),
    edge_radius_rel: positive("edge_radius_rel"),
    label_font_size: positive("label_font_size"),
    label_max_count: positive("label_max_count"),
    color_norm: typeof params.color_norm === "string" ? params.color_norm : refuse("params.color_norm", "not a string"),
  };
}

function radiiOf(value: unknown): Fixture["radii"] {
  const radii = record(value, "radii");
  const read = (name: string): number => {
    const found = num(radii[name], `radii.${name}`);
    if (!(found > 0)) refuse(`radii.${name}`, `${found} is not positive`);
    return found;
  };
  return { R: read("R"), node: read("node"), edge: read("edge") };
}

export function readFixture(text: string): Fixture {
  let parsed: unknown;
  try {
    parsed = JSON.parse(text);
  } catch (error) {
    return refuse("document", error instanceof Error ? error.message : "not JSON");
  }
  const root = record(parsed, "document");
  const nodes = nodesOf(root.nodes);
  return {
    nodes,
    edges: edgesOf(root.edges, nodes.length),
    labels: labelsOf(root.labels, nodes.length),
    params: paramsOf(root.params),
    radii: radiiOf(root.radii),
  };
}
