/** Every action the studio has, in the order the dock shows them. */
import { APPEARANCE_ACTIONS } from "./appearance.ts";
import type { StudioAction } from "./context.ts";
import { EXPORT_ACTIONS } from "./export.ts";
import { FILTER_ACTIONS } from "./filter.ts";
import { PORTABLE_ACTIONS } from "./portable.ts";
import { RUN_ACTIONS } from "./run.ts";
import { SOURCE_ACTIONS } from "./source.ts";
import { VIEW_ACTIONS } from "./view.ts";

export const DOCK_SECTIONS = ["Source", "Layout", "Edges", "Analysis", "Appearance", "Filters", "Export"] as const;

export function studioActions(): readonly StudioAction[] {
  return [
    ...SOURCE_ACTIONS, ...RUN_ACTIONS, ...APPEARANCE_ACTIONS, ...FILTER_ACTIONS, ...EXPORT_ACTIONS, ...PORTABLE_ACTIONS, ...VIEW_ACTIONS,
  ];
}
