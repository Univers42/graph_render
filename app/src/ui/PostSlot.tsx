/**
 * The slot for the parallel `abi-post` branch.
 *
 * That branch adds `Motor#posts`, `Motor#post`, `Motor#analyses` and
 * `Motor#analysis` to the same SDK this studio already loads. The panel is here,
 * marked and inert, and it probes for those methods at runtime rather than
 * importing them: when the branch lands, the probe starts returning true and this
 * component is the only thing that has to change to light up.
 *
 * Nothing here is a stub that pretends to work. Until the methods exist it says
 * so, and the studio is complete without them.
 */

import type { MotorSession } from "../motor/session.ts";

export interface PostSlotProps {
  readonly session: MotorSession | null;
  readonly available: boolean;
}

/** True once the loaded SDK carries the post/analysis surface. Structural, so
 *  this file typechecks against an SDK that does not have it yet. */
export function postSurfaceAvailable(session: MotorSession | null): boolean {
  if (session === null) return false;
  const motor = session as unknown as Record<string, unknown>;
  return typeof motor.posts === "function" && typeof motor.analyses === "function";
}

export function PostSlot(props: PostSlotProps): React.JSX.Element {
  const ready = postSurfaceAvailable(props.session) && props.available;
  return (
    <section className="panel-card panel-card--slot" data-abi-post="pending">
      <h2>Post-processing &amp; analysis</h2>
      {ready ? (
        <p className="hint">the abi-post surface is loaded — its panels land in this slot.</p>
      ) : (
        <>
          <p className="hint">
            reserved for the <code>abi-post</code> branch (<code>Motor#posts</code>, <code>Motor#post</code>,{" "}
            <code>Motor#analyses</code>, <code>Motor#analysis</code>). Not in this build.
          </p>
          <p className="hint">
            Everything the studio shows today comes from the layout run itself; routing, bundling and
            analysis overlays land here when that branch merges.
          </p>
        </>
      )}
    </section>
  );
}
