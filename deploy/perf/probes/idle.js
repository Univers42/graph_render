// Animation callbacks per second while nothing is happening. A parked studio owes zero.
async (args) => {
  const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
  const native = window.requestAnimationFrame.bind(window);
  let callbacks = 0;
  let insideMs = 0;
  window.requestAnimationFrame = (callback) => native((time) => {
    const started = performance.now();
    callback(time);
    callbacks += 1;
    insideMs += performance.now() - started;
  });
  await sleep(args.settleMs);
  callbacks = 0;
  insideMs = 0;
  await sleep(args.seconds * 1000);
  window.requestAnimationFrame = native;
  return {
    seconds: args.seconds,
    callbacksPerSecond: +(callbacks / args.seconds).toFixed(2),
    callbackMsPerSecond: +(insideMs / args.seconds).toFixed(2),
  };
}
