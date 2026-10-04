/**
 * Every action the studio has, in the order the dock shows them. The force actions need the
 * live link, so it is a parameter: the studio passes the bridge's, and a test that has no
 * simulation behind it gets `NO_FORCE_LINK` and sees every force action refuse with a reason.
 */
import { APPEARANCE_ACTIONS } from "./appearance.ts";
import type { StudioAction } from "./context.ts";
import { EXPORT_ACTIONS } from "./export.ts";
import { FILTER_ACTIONS } from "./filter.ts";
import { type ForceLink, NO_FORCE_LINK, forceActions } from "./forces.ts";
import { GROUP_ACTIONS } from "./groups.ts";
import { NODE_ACTIONS } from "./nodes.ts";
import { PORTABLE_ACTIONS } from "./portable.ts";
import { RUN_ACTIONS } from "./run.ts";
import { SOURCE_ACTIONS } from "./source.ts";
import { VIEW_ACTIONS } from "./view.ts";

export const DOCK_SECTIONS = ["Source", "Layout", "Edges", "Analysis", "Appearance", "Filters", "Export", "Forces"] as const;

export function studioActions(link: ForceLink = NO_FORCE_LINK): readonly StudioAction[] {
  return [
    ...SOURCE_ACTIONS, ...RUN_ACTIONS, ...APPEARANCE_ACTIONS, ...FILTER_ACTIONS, ...GROUP_ACTIONS,
    ...EXPORT_ACTIONS, ...PORTABLE_ACTIONS, ...VIEW_ACTIONS, ...NODE_ACTIONS,
    ...forceActions(link),
  ];
}
