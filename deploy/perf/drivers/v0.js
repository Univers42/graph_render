// Driver for the first studio (develop 0d4f9a7): it has no command surface, so the only
// way in is the DOM. Kept to reproduce docs/measurements/studio-perf-baseline.md.
//
// Ponytail: selectors follow that studio's markup (`.panel-card`, its `h2` titles), and
// completion is a fixed wait because every motor call there is synchronous. Wrong for any
// later studio — use the `hook` driver.
(() => {
  const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
  const card = (title) => [...document.querySelectorAll(".panel-card")]
    .find((el) => (el.querySelector("h2")?.textContent ?? "").trim().toLowerCase() === title);
  const setValue = (el, value) => {
    const proto = el instanceof HTMLSelectElement ? HTMLSelectElement.prototype : HTMLInputElement.prototype;
    Object.getOwnPropertyDescriptor(proto, "value").set.call(el, String(value));
    el.dispatchEvent(new Event("input", { bubbles: true }));
    el.dispatchEvent(new Event("change", { bubbles: true }));
  };
  const picker = () => {
    const found = card("layout")?.querySelector("select") ?? null;
    if (found === null) throw new Error("v0 driver: no layout picker");
    return found;
  };
  window.__perf = {
    maxNodes: 2000,
    canvas: () => document.querySelector("canvas.panel__fg"),
    layouts: () => [...picker().options].map((option) => option.value),
    run: async (id) => {
      setValue(picker(), id);
      await sleep(50);
    },
    open: async (nodes, layout) => {
      const data = card("data");
      if (data === undefined) throw new Error("v0 driver: no data card");
      setValue([...data.querySelectorAll("input[type=number]")][1], nodes);
      await sleep(50);
      [...data.querySelectorAll("button")]
        .find((button) => button.textContent.trim().toLowerCase() === "generate").click();
      await sleep(500);
      setValue(picker(), layout);
      await sleep(500);
    },
  };
})()
