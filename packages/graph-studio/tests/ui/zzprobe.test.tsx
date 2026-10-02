import { test } from "node:test";
import * as React from "react";

import { NodeMenu } from "../../src/ui/NodeMenu.tsx";
import { DRAWN, fakeView, studioWith } from "./desk.ts";

type Internals = { H?: unknown };
const key = "__CLIENT_INTERNALS_DO_NOT_USE_OR_WARN_USERS_THEY_CANNOT_UPGRADE";

test("can a dispatcher be stood in for", () => {
  const react = React as unknown as Record<string, Internals | undefined>;
  const internals = react[key];
  console.log("internals", internals === undefined ? "none" : Object.keys(internals).join(","));
  if (internals === undefined) return;
  const before = internals.H;
  internals.H = { useRef: <T,>(value: T) => ({ current: value }), useEffect: () => undefined, useMemo: <T,>(make: () => T) => make() };
  const { studio } = studioWith(DRAWN);
  try {
    const tree = (NodeMenu as unknown as { type: (props: unknown) => unknown }).type({
      studio, ids: DRAWN.meta?.ids ?? null, view: fakeView(), menu: { node: 0, at: { x: 1, y: 2 } }, onClose: () => undefined,
    }) as { props: { children: unknown } };
    console.log("tree", Array.isArray(tree.props.children) ? "list of " + tree.props.children.length : typeof tree.props.children);
    console.log(JSON.stringify(tree).slice(0, 400));
  } catch (error) {
    console.log("threw", String(error).slice(0, 200));
  } finally {
    internals.H = before;
  }
});