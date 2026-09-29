/**
 * The POST panel: which registered pass to apply to the run on screen.
 *
 * The list comes from `Motor#posts()` — the module's own registry, in registry
 * order — so a pass registered after this file was written appears here with no
 * change to the studio (C1), and no post id is written down anywhere in `app/`.
 *
 * A pass is not a second run: it runs on the SAME handle as the layout above it
 * and REPLACES that handle's edge geometry, so Apply replaces the report the
 * canvas draws and Clear re-runs the layout to get the layout's own edges back
 * (the motor has no un-post — see `useStages.ts`).
 */

import type { RunReport } from "../motor/session.ts";

export interface PostPanelProps {
  readonly posts: readonly string[];
  readonly postId: string | null;
  readonly report: RunReport | null;
  readonly available: boolean;
  /** Compare mode runs TWO layouts over the one handle, so only the last one is
   *  the handle's geometry. A pass would silently apply to one pane and leave the
   *  other showing a drawing the handle no longer holds, so the panel is closed
   *  there and says why. */
  readonly compare: boolean;
  readonly onPick: (id: string) => void;
  readonly onApply: (id: string) => void;
  readonly onClear: () => void;
}

/** The pass the picker falls back to: the one on screen, else the first
 *  registered. Never a hard-coded id. */
function selected(props: PostPanelProps): string {
  return props.postId ?? props.report?.postId ?? props.posts[0] ?? "";
}

export function PostPanel(props: PostPanelProps): React.JSX.Element {
  const value = selected(props);
  const applied = props.report?.postId ?? null;
  const ready = props.available && props.report !== null && value !== "" && !props.compare;
  return (
    <section className="panel-card">
      <h2>Post-processing</h2>
      <label className="field">
        <span>pass</span>
        <select
          value={value}
          disabled={props.posts.length === 0}
          onChange={(event) => props.onPick(event.target.value)}
        >
          {props.posts.length === 0 && <option value="">(no post passes — motor unavailable)</option>}
          {props.posts.map((id) => (
            <option key={id} value={id}>
              {id}
            </option>
          ))}
        </select>
      </label>
      <div className="row">
        <button type="button" disabled={!ready} onClick={() => props.onApply(value)}>
          Apply
        </button>
        <button
          type="button"
          className="secondary"
          disabled={!props.available || applied === null || props.compare}
          onClick={props.onClear}
        >
          Clear
        </button>
      </div>
      <p className="hint">
        {props.posts.length} pass(es) registered by the module
        {applied === null
          ? " — showing the layout's own edges"
          : ` — showing ${applied}'s edges, ${props.report?.edgeKind}`}
      </p>
      {props.compare ? (
        <p className="hint">
          compare mode runs two layouts over the one handle, and a pass can only replace the last one's
          edges — turn compare off to apply one
        </p>
      ) : (
        <p className="hint">
          A pass runs on the same handle as the layout and replaces its edge geometry; Clear re-runs the
          layout, since a pass cannot be undone in place.
        </p>
      )}
    </section>
  );
}
