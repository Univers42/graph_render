/**
 * The release a mount that threw part-way still owes (`src/mount.ts`, `release`): `unmount`'s steps,
 * in `unmount`'s order, on the parts that exist — and nothing at all when nothing was made.
 *
 * Option (b) of the job, and why: `mount` needs `document`, `attachShadow`, `CSSStyleSheet` and the
 * React root, and this package has no DOM library in any of its dependency sets, so it cannot run
 * under `node:test` and no such library may be installed. The helper is exported and driven here
 * with fakes instead, which is what the end-to-end case would have asserted through it: that the
 * worker, the view and every watcher made before the throw are taken back.
 */
import assert from "node:assert/strict";
import { test } from "node:test";

import { type Building, release } from "../../src/mount.ts";

/** `unmount`'s order, and the only order `release` may take them in. */
const ORDER = ["unhost", "root", "unwatchArea", "unwatch", "bridge", "studio", "view"] as const;

/**
 * `made` of the parts, each standing for its own release step. A fake carries the single member
 * `release` calls on it, which is all `release` asks of a real one; a part `made` does not name is
 * left out, which is what a `throw` part-way through `mount` leaves.
 */
function recording(made: readonly string[]): { readonly parts: Building; readonly steps: string[] } {
  const steps: string[] = [];
  const step = (name: string) => (): void => void steps.push(name);
  const parts: Building = {};
  if (made.includes("unhost")) parts.unhost = step("unhost");
  if (made.includes("root")) parts.root = { unmount: step("root") };
  if (made.includes("unwatchArea")) parts.unwatchArea = step("unwatchArea");
  if (made.includes("unwatch")) parts.unwatch = step("unwatch");
  if (made.includes("bridge")) parts.bridge = { destroy: step("bridge") };
  if (made.includes("studio")) parts.studio = { destroy: step("studio") };
  if (made.includes("view")) parts.view = { destroy: step("view") };
  return { parts, steps };
}

test("release takes back every part of a mount in unmount's order, only the ones made, and nothing on an empty one", () => {
  const whole = recording(ORDER);
  release(whole.parts);
  assert.deepEqual(whole.steps, ORDER);

  // A throw after the root was made and before the area watcher: what was made is released, the
  // rest is not waited for, and the order of those that exist is `unmount`'s.
  const partway = recording(["studio", "view", "bridge", "root", "unwatch"]);
  release(partway.parts);
  assert.deepEqual(partway.steps, ["root", "unwatch", "bridge", "studio", "view"]);

  const nothing = recording([]);
  release(nothing.parts);
  assert.deepEqual(nothing.steps, []);
});