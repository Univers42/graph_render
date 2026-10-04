# Layout parameters in the dock: what the motor publishes, and what a change costs

Job `ux-params-dock`, 2026-10-03. The ABI is `docs/decisions/layout-params.md`; this is the half
that puts it in the product: the dock's **Layout settings** panel, the console words, and the
numbers below.

## What is on screen

The panel is built from the schema the motor sent and from nothing else. `state.schemas` holds
one `LayoutParamSpec[]` per layout id, read through the worker (`gm_layout_params`, once per
layout, never re-read per drawing), and `ui/paramSpecs.ts` turns each spec into a knob of the
studio's own: a `float` asks for a slider, an `int` takes the plain number field and a `bool` the
plain switch, all through `controlOf` and the controls that already existed. There is no layout id
and no parameter name in studio source; `state/settings.ts`'s `params` member is a map of maps
whose keys are data.

A commit is one dispatch of `layout.params` (the values as JSON), and the panel collapses whatever
arrives inside one frame into that one dispatch (`ui/oneRun.ts`, the same frame scheduler the live
bar uses). The run it asks for cancels whatever is in flight on the way — `pipeline.apply` calls
the same `client.cancel()` that `view.cancel` does.

Values live in `Settings.params`, keyed by layout id, so they are persisted per source
(`state/persist.ts`), exported and imported whole (`state/portable.ts`), and kept per layout: each
layout remembers its own, and `layoutreset` gives one layout back to the motor's published
defaults — the run at the published defaults is byte-identical to the run with no values at all
(`a_run_at_the_defaults_is_the_registered_run` in graph-core, and `params-reset` in the gate below
compares the digests).

## Measured: from a slider move to the first new frame

One measurement, one host, 2026-10-03, on the dev server of this worktree (Chromium 154 headless,
canvas2d at 2 000 nodes and WebGL2 at 20 000). The clock starts at the `pointerdown` on the slider
and stops at the first `view.on("frame")` the studio raises after it; the marks after that are the
transition's own frames. The gesture is a real click on the slider's track, so it is the whole
path: control → panel → registry → worker → `gm_run` with a parameter buffer → snapshot → paint.

| nodes | links | layout | parameter | value | layout | to first frame | transition frames | overlay |
|---:|---:|---|---|---|---:|---:|---|---|
| 2 000 | 3 998 | `layout.dag.sugiyama` | `layer_spacing` | 1.0625 → 512.0625 | 42.4 ms | **82.0 ms** | 88.9, 97.8, 114.6, 131.3 ms | `2000 n · 3998 e · idle, last 60 fps · <1 ms · canvas2d` |
| 20 000 | 39 998 | `layout.dag.sugiyama` | `layer_spacing` | 1.0625 → 512.0625 | 211.0 ms | **281.1 ms** | 294.4, 362.2, 372.9 ms | `20000 n · 39998 e · idle, last 33 fps · 9 ms · webgl2` |

What the 40 ms and the 70 ms on top of the layout are: one frame of debounce, the worker's round
trip, the decode of the snapshot, and the first paint of a frame the view has not drawn before. The
layout itself is the motor's own number, off the run report (`run.layoutMs`), and it is most of
the total — which is the answer to the question the dock asks: at 20 000 nodes the parameter change
is a re-run of the layout, and the studio's own cost is about a tenth of it.

Caveat: this is a container on a shared host with a software rasteriser, so the absolute numbers
are that host's and the frame times in the overlay (`<1 ms`, `9 ms`) are the canvas's, not the
studio's. What is portable is the shape: the overhead over the layout grows far more slowly than
the layout does, and no measurement here covers a layout run over a minute (`LIVE_NODES` puts the
big force layouts on the live loop instead, where a parameter change has no effect at all — see
the limitation below).

## The gate

`scripts/studio-params.sh` (`deploy/nav/params.py`) serves the built studio on 127.0.0.1 and drives
it in headless Chromium with real mouse input on the dock's own controls. Seven rows, all green:

| row | claim |
|---|---|
| `params-panel` | a force layout publishing three parameters shows three controls, two sliders and a number field |
| `params-slider` | a dragged slider changes the value, the run's digest and the pixels |
| `params-one-run` | a 20-step drag of one slider asks the motor for exactly one run |
| `params-console` | `layoutset iterations 2` writes the value, costs one run and redraws |
| `params-reset` | after a change, `layoutreset` redraws the published defaults, the same digest as the first run |
| `params-refused` | `layoutset threshold 5` is refused by name and the drawing is untouched |
| `params-layered` | on `layout.dag.sugiyama` the panel shows only `layer_spacing`, and moving it redraws the layers |

The negative control is `STUDIO_PARAMS_BREAK=1`: the pointer never reaches the control, which is
the gesture every row here depends on. Five of the seven rows go red (`params-slider`,
`params-one-run`, `params-console`, `params-reset`, `params-layered`) and the gate exits 1; the two
that stay green are the rows that claim no change — the panel's own contents, and a refusal.

After the merge with develop 96b81e43 (2026-10-04), four rows went red: `params-slider`,
`params-one-run`, `params-reset` and `params-layered`, with 0 layout calls for 20 pointer moves. The
studio was not at fault. Develop's taller Layout section put the settings below the dock's visible
area (threshold track top at y = 963 in a 720 px viewport, measured with the pw MCP), so the press
landed outside the control. The same drag on the control scrolled into view committed one run
(0.0001 → 0.997). `paramspage.py` now scrolls the control and the section header into view before
pressing, as a hand would. Re-run: 7 of 7 PASS. `STUDIO_PARAMS_BREAK=1` exits 1 with 5 FAIL.

## The pw pass

Driven against this worktree's dev server with the `pw` MCP server, at 1280x720. One parameter of
a layered layout and one of a force layout, each changed with a real click on its slider.

| screenshot | what it shows |
|---|---|
| `ux-params-dock/layered-sugiyama-400.png` | `layout.dag.sugiyama`, `layer_spacing` at 512 (the published default is 1): one slider in the panel, the layers drawn 512 units apart, `layout 12 ms`, digest `b8ed849a` |
| `ux-params-dock/force-spring-400.png` | `layout.force.spring` after its `scale` slider was clicked to 500000: the panel has followed the layout and now shows `iterations` (number), `threshold` and `scale` (sliders), digest `0dba9127` |
| `ux-params-dock/layered-sugiyama-20000.png` | the same layered parameter on 20 000 nodes: WebGL2, 33 fps, 9 ms a frame, `layout 211 ms`, digest `7bfc5ee6` |

The three readings the brief asks for, after both changes:

- **store error**: `null`. Nothing is raised and nothing is stale.
- **console exceptions**: none. Seven console messages in the whole session, 0 errors, 0
  exceptions; four are Chromium's own GL driver notices (`GPU stall due to ReadPixels`,
  `GL_CLOSE_PATH_NV`) from the WebGL2 layer on the software rasteriser, which the driver prints
  and the studio never sees.
- **overlay text**: the HUD's own numbers, with no error line —
  `20000 n · 39998 e · idle, last 33 fps · 9 ms · webgl2 · layout 211 ms · 7bfc5ee6`.

Two things worth recording from the pass:

- The log holds one refusal, and it is the probe's own: `synthetic nodes 2000` (the console's
  positional form for a four-parameter action) is refused with `` `nodes` must be a whole number,
  not "nodes" ``. `synthetic nodes=2000` is the named form and works. The refusal is correct and
  the store error was clear of it by the next command.
- A large parameter change can put the drawing outside the viewport, because the camera is kept
  across a layout change (as it is across a filter change). `f` fits it again: on the 400-node
  layered drawing the fit moved the camera from `x 520 y 131 ×0.036` to
  `x 519.76 y 130.50 ×0.03595`. Not a defect and not a change here — the camera is the viewer's,
  and the studio's settings document says so — but it is the first thing a user meets after a big
  knob.

## What this does not reach

- **A force layout past `LIVE_NODES` takes no parameter.** `planRun` (`motor/settle.ts`) runs such
  a graph as a scatter and reports `layout.force.particle_mesh`, which publishes nothing; the
  session then sends the empty buffer and the report says so with an empty `params`, which is what
  the next plan is compared against. The panel follows honestly: after the run the current layout
  is the mesh, and the panel says the mesh publishes no parameters. Ponytail: the eleven other
  parameters of that force layout are reachable only under 5 000 nodes through this panel; the
  live force session (`docs/decisions/live-force-session.md`) is the surface for them. Direction:
  the substituted run could carry the asked layout's values to a layout that reads them. Escape
  hatch: `planOf` compares the values of the layout that actually ran, so the substitution costs
  no second run and no wrong picture.
- **The schema is read once per layout and never re-read.** A motor whose schema changed under a
  live module would keep the first answer; the ABI version (`gm_abi_version` = 2) is what catches
  that case, and a module that cannot answer at all is a note on the run rather than a failed
  drawing (`the parameters of <layout> are unknown: …`).
- **No value is range-checked twice.** The studio checks the name against the schema and the value
  against the schema's own bounds, once, in `actions/params.ts::checkedValue`, reached through the
  registry's `accept` so the dock's control and the typed line are refused by the same rule. The
  motor still refuses an out-of-range value itself (`ParamOutOfRange`), which is the boundary's own
  check and is left where it is.
