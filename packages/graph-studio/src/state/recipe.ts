/**
 * A drawing as a document: the settings that made it, what it came out as, and the
 * commands that led there. Replaying applies the settings and compares what comes out;
 * the commands are the record of how, for whoever reads the file.
 */
import type { StudioState } from "./model.ts";
import { type Fields, SettingsRefusal, fieldsOf, numberOf, textOrNull, textsOf } from "./read.ts";
import { type Settings, readSettings } from "./settings.ts";

export interface Expectation {
  /** sha256 of the snapshot's bytes; `null` where none could be taken. */
  readonly digest: string | null;
  readonly nodeCount: number;
  readonly edgeCount: number;
}

export interface Recipe {
  readonly version: 1;
  readonly settings: Settings;
  readonly expect: Expectation;
  readonly log: readonly string[];
}

/** The motor drew something other than what the recipe recorded. */
export class RecipeMismatch extends Error {
  constructor(message: string) {
    super(message);
    this.name = "RecipeMismatch";
  }
}

const COUNT = { min: 0, max: 4294967295, whole: true } as const;

export function expectationOf(state: StudioState): Expectation | null {
  if (state.graph === null || state.run === null) return null;
  return { digest: state.run.digest, nodeCount: state.graph.nodeCount, edgeCount: state.graph.edgeCount };
}

export function recipeOf(state: StudioState): Recipe {
  const expect = expectationOf(state);
  if (expect === null) throw new RecipeMismatch("nothing is drawn, so there is nothing to record");
  const log = state.log.filter((entry) => entry.ok).map((entry) => entry.command);
  return { version: 1, settings: state.settings, expect, log };
}

function parsed(text: string): unknown {
  try {
    const value: unknown = JSON.parse(text);
    return value;
  } catch (error) {
    throw new SettingsRefusal("recipe", `not JSON (${error instanceof Error ? error.message : String(error)})`);
  }
}

function readExpectation(value: unknown, at: string): Expectation {
  const fields: Fields = fieldsOf(value, at, ["digest", "nodeCount", "edgeCount"]);
  return {
    digest: textOrNull(fields, at, "digest"),
    nodeCount: numberOf(fields, at, "nodeCount", COUNT),
    edgeCount: numberOf(fields, at, "edgeCount", COUNT),
  };
}

export function readRecipe(text: string): Recipe {
  const fields = fieldsOf(parsed(text), "recipe", ["version", "settings", "expect", "log"]);
  if (fields["version"] !== 1) throw new SettingsRefusal("recipe.version", "not 1, the only version this studio reads");
  return {
    version: 1,
    settings: readSettings(fields["settings"], "recipe.settings"),
    expect: readExpectation(fields["expect"], "recipe.expect"),
    log: textsOf(fields, "recipe", "log"),
  };
}

function short(digest: string): string {
  return digest.slice(0, 8);
}

/** What could not be compared, as notes. Throws when something compared and differed. */
export function checkRecipe(expect: Expectation, drawn: Expectation): readonly string[] {
  if (expect.nodeCount !== drawn.nodeCount || expect.edgeCount !== drawn.edgeCount) {
    throw new RecipeMismatch(
      `the recipe expects ${expect.nodeCount} nodes and ${expect.edgeCount} links; this drew ${drawn.nodeCount} and ${drawn.edgeCount}`,
    );
  }
  if (expect.digest === null) return ["digest NOT checked: the recipe holds none"];
  if (drawn.digest === null) return ["digest NOT checked: this platform offers no digest"];
  if (expect.digest !== drawn.digest) {
    throw new RecipeMismatch(`the recipe expects digest ${short(expect.digest)}…; this drew ${short(drawn.digest)}…`);
  }
  return [];
}
