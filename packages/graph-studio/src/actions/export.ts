/** What leaves the studio: the picture, the snapshot's bytes, and the recipe for both. */
import type { StudioState } from "../state/model.ts";
import { checkRecipe, expectationOf, readRecipe, recipeOf } from "../state/recipe.ts";
import { type StudioAction, type StudioContext, textArg } from "./context.ts";
import { ActionRefusal, type Outcome } from "./registry.ts";

function drawn(state: StudioState): string | null {
  return state.run === null ? "nothing is drawn" : null;
}

function nameFor(state: StudioState, stem: string, extension: string): string {
  const digest = state.run?.digest ?? null;
  return `${stem}-${digest === null ? "undigested" : digest.slice(0, 8)}.${extension}`;
}

function saved(context: StudioContext, name: string, data: Blob): Outcome {
  context.save(name, data);
  return { message: `${name} (${data.size} bytes)`, digest: context.state().run?.digest ?? null };
}

async function replay(context: StudioContext, text: string): Promise<Outcome> {
  const recipe = readRecipe(text);
  const outcome = await context.apply(recipe.settings);
  const now = expectationOf(context.state());
  if (now === null) throw new ActionRefusal("unavailable", "the recipe drew nothing");
  const unchecked = checkRecipe(recipe.expect, now);
  const verdict = unchecked.length === 0 ? "the digest matches" : "replayed";
  return { message: `${verdict} · ${outcome.message}`, digest: now.digest, notes: [...unchecked, ...(outcome.notes ?? [])] };
}

const png: StudioAction = {
  id: "export.png", alias: "png", title: "Save the picture", section: "Export", params: [],
  available: drawn,
  run: async (context) => saved(context, nameFor(context.state(), "graph", "png"), await context.view.toPNG()),
};

const snapshot: StudioAction = {
  id: "export.snapshot", alias: "snapshot", title: "Save the snapshot", section: "Export", params: [],
  available: drawn,
  run: (context) => {
    const bytes = context.bytes();
    if (bytes === null) throw new ActionRefusal("unavailable", "nothing is drawn");
    return saved(context, nameFor(context.state(), "graph", "gmsn"), new Blob([bytes.slice()], { type: "application/octet-stream" }));
  },
};

const recipe: StudioAction = {
  id: "export.recipe", alias: "recipe", title: "Save the recipe", section: "Export", params: [],
  available: drawn,
  run: (context) => {
    const text = JSON.stringify(recipeOf(context.state()), null, 2);
    return saved(context, nameFor(context.state(), "recipe", "json"), new Blob([text], { type: "application/json" }));
  },
};

const replayed: StudioAction = {
  id: "recipe.replay", alias: "replay", title: "Replay a recipe", section: "Export",
  params: [{ name: "text", kind: "text", title: "Recipe JSON", control: "file", value: () => "" }],
  run: (context, args) => replay(context, textArg(args, "text")),
};

export const EXPORT_ACTIONS: readonly StudioAction[] = [png, snapshot, recipe, replayed];
