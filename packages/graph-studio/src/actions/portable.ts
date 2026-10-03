/** Settings out and in, and a reset for each panel that keeps its own. */
import { PARAMS_SECTION } from "./params.ts";
import { SECTIONS, type Section, exportSettings, importSettings, resetSection } from "../state/portable.ts";
import type { StudioAction } from "./context.ts";
import { textArg } from "./context.ts";

const exported: StudioAction = {
  id: "settings.export", alias: "exportsettings", title: "Save the settings", section: "Export", params: [],
  run: (context) => {
    const text = exportSettings(context.state().settings);
    context.save("settings.json", new Blob([text], { type: "application/json" }));
    return { message: `settings.json (${text.length} characters)` };
  },
};

const imported: StudioAction = {
  id: "settings.import", alias: "importsettings", title: "Load settings", section: "Export",
  params: [{ name: "text", kind: "text", title: "Settings JSON", control: "file", value: () => "" }],
  run: (context, args) => context.apply(importSettings(textArg(args, "text"))),
};

const PANELS: Readonly<Record<Section, string>> = {
  appearance: "Appearance", filter: "Filters", layout: "Layout", edges: "Edges",
  analysis: "Analysis", params: PARAMS_SECTION,
};

function resetOf(section: Section): StudioAction {
  return {
    id: `settings.reset.${section}`, alias: `reset${section}`, title: "Reset this panel", section: PANELS[section], params: [],
    run: (context) => context.apply(resetSection(context.state().settings, section)),
  };
}

export const PORTABLE_ACTIONS: readonly StudioAction[] = [exported, imported, ...SECTIONS.map(resetOf)];
