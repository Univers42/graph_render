/**
 * The edge draw's own GPU time, read back through `EXT_disjoint_timer_query_webgl2`.
 *
 * One `TIME_ELAPSED` query runs around each `drawElements` of the edges (draw.ts) and its
 * result is read back when a later `end()` drains the pending ones, so the readback never
 * stalls the frame. Where the browser gives no such extension — every software rasteriser,
 * every headless container — `edgeTimerOf` hands back the silent timer below: one
 * `getExtension` at layer creation and every later call a null check, nothing else.
 *
 * Ponytail: a query measures what the GPU did between begin and end — the edge draw alone,
 * not the blit over the ground and not Chrome's compositing — and a disjoint result (another
 * context moved the clock) is dropped rather than counted, so this under-counts a frame
 * instead of over-counting it, and it says nothing about a canvas2d frame at all.
 */
export interface EdgeTimer {
  /** Starts a query, unless one is already open or the context is lost. */
  begin(): void;
  /** Closes the open query and reads back the results that have landed; a no-op with none open. */
  end(): void;
  /** Edge-draw milliseconds accumulated so far: monotonic, and 0 without the extension. */
  ms(): number;
}

/** The timer a layer without the extension gets: every call does nothing and `ms()` is 0. */
const SILENT: EdgeTimer = { begin: () => undefined, end: () => undefined, ms: () => 0 };

/** The extension's own enums, which the spec fixes: the query target, and the disjoint flag. */
const TIME_ELAPSED_EXT = 0x88bf;
const GPU_DISJOINT_EXT = 0x88bb;

/** The edge draw's timer on this context, or the silent one when it cannot time anything. */
export function edgeTimerOf(gl: WebGL2RenderingContext): EdgeTimer {
  const found: unknown = gl.getExtension("EXT_disjoint_timer_query_webgl2");
  return found === null || found === undefined ? SILENT : timerOn(gl);
}

function timerOn(gl: WebGL2RenderingContext): EdgeTimer {
  let active: WebGLQuery | null = null;
  let total = 0;
  const pending: WebGLQuery[] = [];
  // A query answers a frame or two after it ends, so every end reads back what has landed.
  // A disjoint result belongs to another context's clock: dropped, never counted.
  function drain(): void {
    for (let at = pending.length - 1; at >= 0; at -= 1) {
      const query = pending[at] ?? null;
      if (query === null) continue;
      if (!gl.getQueryParameter(query, gl.QUERY_RESULT_AVAILABLE)) continue;
      pending.splice(at, 1);
      const disjoint: unknown = gl.getParameter(GPU_DISJOINT_EXT);
      if (!disjoint) {
        const nanos: unknown = gl.getQueryParameter(query, gl.QUERY_RESULT);
        if (typeof nanos === "number") total += nanos / 1e6;
      }
      gl.deleteQuery(query);
    }
  }
  return {
    begin(): void {
      if (active !== null || gl.isContextLost()) return;
      const query = gl.createQuery();
      gl.beginQuery(TIME_ELAPSED_EXT, query);
      active = query;
    },
    end(): void {
      if (active === null) return;
      gl.endQuery(TIME_ELAPSED_EXT);
      pending.push(active);
      active = null;
      drain();
    },
    ms(): number {
      return total;
    },
  };
}
