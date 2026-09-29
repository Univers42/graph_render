/**
 * A SciGraphs preset as the painter's theme: the single encode of the linear triples
 * api/render.py:45-101 stores, and the label metrics of the gallery pipeline.
 *   SciGraphs/api/render.py:45-101 (PRESETS: background, color, edge_color)
 *   SciGraphs/docs/examples/05-reproducible-pipeline.qmd:121-145 (the gallery values)
 *   SciGraphs/core/visualization/text_overlay.py:533-562 (white text, the 0.6 box, 3 px pad)
 */
import { cssOf } from "../colour/srgb.ts";
import { LABEL_BOX, LABEL_FONT_PX, LABEL_PADDING_PX, type Look } from "./presets.ts";
import type { Theme } from "../theme.ts";

/** The label text is white on every preset (text_overlay.py:551-556). */
const LABEL_COLOUR = "#ffffff";

/** The source asks for DejaVu Sans by name; the family after it is the fallback. */
const LABEL_FAMILY = '"DejaVu Sans", sans-serif';

/**
 * The drawn line box of a label. The source sets a font size and leaves the rest to the
 * renderer's own text layout (executor.py:556-562).
 *
 * Ponytail: 1.2 of the font size is the line box this painter bakes, which is the usual
 * single-line value; a face with a taller default line box would be clipped by a pixel or
 * two at 26 px. The escape hatch is to set `labelHeight` on the theme.
 */
const LINE_BOX = 1.2;

/**
 * Outside a lit neighbourhood the source draws nothing dimmed, so this is the studio's own
 * affordance value.
 *
 * Ponytail: 0.16 is the studio's dark and light theme value (theme.ts:31,43) rather than
 * anything SciGraphs states, so a look and a studio theme dim by the same amount; a theme
 * that wants another has to say so. The escape hatch is `dimAlpha` on the theme.
 */
const DIM_ALPHA = 0.16;

/** The sprite box of a label: the line box plus the padding on both sides (text_overlay.py:547). */
export const LABEL_BOX_PX = Math.ceil(LABEL_FONT_PX * LINE_BOX + LABEL_PADDING_PX * 2);

/** The fill and the clear space of the box the source draws behind its labels. */
export const LABEL_BACKDROP = { fill: LABEL_BOX, padding: LABEL_PADDING_PX } as const;

export function lookTheme(look: Look): Theme {
  return {
    background: cssOf(look.background),
    edge: cssOf(look.edge),
    // The source has no highlight colour: the flat node colour is the only accent a
    // preset carries (api/render.py:50), so the lit edges and the ring share it.
    // Ponytail: on a metric-coloured look that is slate's borrowed grey, not the node's own
    // colour, so the ring around a lit node is a constant rather than a match; the escape
    // hatch is to pass a theme in.
    edgeLit: cssOf(look.node),
    ring: cssOf(look.node),
    rim: cssOf(look.background),
    label: LABEL_COLOUR,
    labelHalo: LABEL_BOX,
    labelBox: LABEL_BACKDROP,
    labelHeight: LABEL_BOX_PX,
    labelFont: `${LABEL_FONT_PX}px ${LABEL_FAMILY}`,
    dimAlpha: DIM_ALPHA,
  };
}
