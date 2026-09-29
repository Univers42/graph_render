/** The one way an appearance action changes the drawing: a new settings document, no motor. */
import { type Appearance, withAppearance } from "../state/settings.ts";
import type { StudioContext } from "./context.ts";
import { ActionRefusal, type Outcome } from "./registry.ts";

export function look(context: StudioContext, patch: Partial<Appearance>): Outcome {
  const byAnalysis = patch.colourBy === "analysis" || patch.sizeBy === "analysis";
  if (byAnalysis && context.state().analysis === null) {
    throw new ActionRefusal("unavailable", "no analysis has run; pick one with `analysis` first");
  }
  return context.look(withAppearance(context.state().settings, patch));
}

