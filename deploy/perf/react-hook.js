// Counts React commits, and the components each commit rendered, through the global hook
// React looks for when react-dom loads (the one React DevTools installs). Production builds
// call it too, so the studio is measured as shipped. Injected before any page script
// (run.py, Page.addScriptToEvaluateOnNewDocument); the frame probe reads window.__reactCommits.
//
// A commit is walked the way React DevTools walks it: a subtree whose child pointer is the
// one the previous tree had was bailed out without being cloned, so it is not entered.
//
// Caveat: "rendered" reads React's internal PerformedWork flag (bit 0 of fiber.flags, the
// same value in React 16 to 19.2) on fibers whose type is a function, so a forwardRef or a
// lazy component is not counted, and a React that renamed the flag would read 0 renders
// (an under-count, which makes a budget row pass that should fail: re-check the flag
// against react-reconciler's ReactFiberFlags.js after a React upgrade).
(() => {
  const PERFORMED_WORK = 1;
  const counts = { commits: 0, rendered: 0 };
  const walk = (root) => {
    const stack = [root];
    while (stack.length > 0) {
      const fiber = stack.pop();
      if (typeof fiber.type === "function" && (fiber.flags & PERFORMED_WORK) !== 0) counts.rendered += 1;
      if (fiber.child === null || fiber.child === fiber.alternate?.child) continue;
      for (let child = fiber.child; child !== null; child = child.sibling) stack.push(child);
    }
  };
  window.__reactCommits = counts;
  window.__REACT_DEVTOOLS_GLOBAL_HOOK__ = {
    supportsFiber: true,
    renderers: new Map(),
    inject: () => 1,
    checkDCE: () => {},
    onScheduleFiberRoot: () => {},
    onCommitFiberRoot: (_renderer, root) => {
      counts.commits += 1;
      walk(root.current);
    },
    onCommitFiberUnmount: () => {},
    onPostCommitFiberRoot: () => {},
  };
})();
