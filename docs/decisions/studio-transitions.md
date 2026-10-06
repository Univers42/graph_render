# Studio transitions: what moves, what fades, what cuts (2026-10-06)

Recorded by the studio-speed branch. Before it, a layout switch eased the nodes for 600 ms, and
every other change cut to the new picture. A routed edge was drawn straight until the move ended.
A restyle, a theme or another document replaced the picture in one frame.

| Change | Before | Now | Where |
|---|---|---|---|
| Layout switch, same graph | nodes eased over 600 ms; routed edges drawn straight while moving; the first frame jumped to the new fit's camera | 400 ms (`TRANSITION_MS`); routed edges bend from the old route to the new one; the first frame is the picture already on screen (`placeStart`) | `transition.ts`, `canvas2d/morph.ts`, `canvas2d/tween.ts` |
| Another document | cut | a 220 ms cross-fade, and nodes whose ids the two documents share move from where they were drawn | `canvas2d/crossfade.ts`, graph-studio `studio/carry.ts` |
| Look, theme, analysis colours | cut | 220 ms cross-fade (`View.crossFade`) | graph-studio `studio/pipeline.ts` |
| A restyle whose six inputs are unchanged | a full style and scene rebuild | skipped | graph-studio `studio/pipeline/styleIn.ts` |

## Why a cross-fade and not a per-node colour tween

A per-node colour and size tween would mean a second set of style columns and a blend in every
painter: Canvas2D, the WebGL2 layer and the 3D path. The cross-fade copies the canvas once and draws
it over the next repaints, so it does not depend on which painter drew the frame. Its limit is in its
`Caveat:`: the copy is in screen pixels, so a pan during the 220 ms slides the new drawing under a
copy that stays put.

## Measured (studio-perf, SwiftShader, same host, d2c34b67 → studio-speed)

| Row | Before | After |
|---|---|---|
| React renders while opening, 120 nodes at DPR 1 | 1114 | 624 |
| `perf-block`, the worst main-thread block of any layout switch | 16 ms | 9 ms |
| Layout switch, 2 000 nodes (`studio-probe.sh transition 2000 canvas2d`) | — | 24 frames in 406 ms, gap p95 16.67 ms |
| Layout switch, 20 000 nodes | — | 24 frames in 434 ms, gap p95 16.67 ms |

`perf-fps` fails at 2 000 nodes at DPR 2 on both commits (5.5 and 5.6 fps against a floor of 14).
That row rasterises in software, on a shared host. It is not a result of this branch, and this branch
does not fix it.
