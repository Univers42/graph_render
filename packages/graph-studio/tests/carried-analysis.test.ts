/**
 * A load re-runs the analysis the settings carry (`studio/plan.ts`), and the pipeline awaits it.
 * Betweenness and closeness are super-linear, so a persisted pick left on hung a million-node
 * load with nothing on screen to say why. A carried pick past the interactive ceiling is held
 * back with a note; a pick the user makes now always runs.
 */
import assert from "node:assert/strict";
import { test } from "node:test";

import type { MotorClient } from "../src/motor/client.ts";
import type { AnalysisReport } from "../src/motor/protocol.ts";
import { DEFAULT_SETTINGS, withSettings } from "../src/state/settings.ts";
import { ANALYSIS_CEILING } from "../src/studio/plan.ts";
import { desk, scriptedClient } from "./desk.ts";

const BETWEENNESS = "analysis.centrality.betweenness";

function report(id: string): AnalysisReport {
  return { id, kind: "f64", values: new Float64Array(3), converged: null, modularity: null, max: null, ms: 1 };
}

/** The scripted motor, reporting `nodes` on a load and answering every analysis it is asked for. */
function measuring(nodes: number): { readonly client: MotorClient; readonly asked: string[] } {
  const asked: string[] = [];
  const scripted = scriptedClient();
  const client: MotorClient = {
    ...scripted,
    load: async (source) => ({ ...(await scripted.load(source)), nodeCount: nodes }),
    analysis: (id) => { asked.push(id); return Promise.resolve(report(id)); },
  };
  return { client, asked };
}

async function opened(nodes: number, analysis: string) {
  const motor = measuring(nodes);
  const made = desk(motor.client, withSettings(DEFAULT_SETTINGS, { analysis }));
  const entry = await made.studio.start();
  return { motor, made, entry };
}

test("carried_betweenness_past_the_ceiling_is_held_back", async () => {
  const { motor, made, entry } = await opened(ANALYSIS_CEILING + 1, BETWEENNESS);
  assert.equal(entry.error, null);
  assert.deepEqual(motor.asked, []);
  assert.equal(made.studio.store.get().analysis, null);
  assert.equal(made.studio.store.get().settings.analysis, null);
  assert.ok(entry.notes.some((note) => note.includes(BETWEENNESS) && note.includes("not re-run")), entry.notes.join("\n"));
});

test("carried_betweenness_at_the_ceiling_runs", async () => {
  const { motor, made } = await opened(ANALYSIS_CEILING, BETWEENNESS);
  assert.deepEqual(motor.asked, [BETWEENNESS]);
  assert.equal(made.studio.store.get().analysis?.id, BETWEENNESS);
});

test("a_pick_after_the_hold_runs", async () => {
  const { motor, made } = await opened(ANALYSIS_CEILING + 1, BETWEENNESS);
  await made.pipeline.apply(withSettings(made.studio.store.get().settings, { analysis: BETWEENNESS }));
  assert.deepEqual(motor.asked, [BETWEENNESS]);
  assert.equal(made.studio.store.get().analysis?.id, BETWEENNESS);
});

test("closeness_is_held_and_louvain_is_not", async () => {
  const closeness = await opened(ANALYSIS_CEILING + 1, "analysis.centrality.closeness");
  assert.deepEqual(closeness.motor.asked, []);
  const louvain = await opened(ANALYSIS_CEILING + 1, "analysis.communities.louvain");
  assert.deepEqual(louvain.motor.asked, ["analysis.communities.louvain"]);
});
