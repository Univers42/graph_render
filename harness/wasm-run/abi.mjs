// The arm's view of the module: framed reads, the three registries, and the stage table.
// Split out of `harness/wasm-run.mjs` by the house's 300-line limit (the review's m40).
//
// Nothing here consults a literal id: every layout, analysis and POST name is resolved
// through the module's own registry (C1), and the one stage that is not a registered
// capability is keyed off `SHIM_LAYOUT` — the same constant the C20 check in `./hash.mjs`
// reads, so the two sides of that comparison cannot drift apart (m37/m38).

import { refuse, SHIM_LAYOUT, TRANSPORT_STAGE } from "./lib.mjs";

/// Everything one mode drives, bound to one module's `exports`.
///
/// One instance per module rather than module-level state, so a second module can be
/// loaded in the same process (the unit test loads a deliberately-truncated one) without
/// the first module's registries leaking into it. Each registry is read once and cached:
/// the arm scans each of them once per invocation.
export function createAbi(exports) {
  return new Abi(exports);
}

class Abi {
  #exports;
  #layouts = null;
  #analyses = null;
  #posts = null;
  #stageBytes = null;

  constructor(exports) {
    this.#exports = exports;
  }

  /// Exports return a pointer to [len: u32 LE][len bytes]; 0 means the motor refused.
  /// `.slice()` copies out of wasm memory, so a later `memory.grow` cannot detach the
  /// bytes under a digest that has not been taken yet.
  framed(ptr) {
    if (ptr === 0) {
      refuse("export returned 0: the motor refused (non-finite value or oversize buffer)");
    }
    const len = new DataView(this.#exports.memory.buffer).getUint32(ptr, true);
    return new Uint8Array(this.#exports.memory.buffer, ptr + 4, len).slice();
  }

  /// `name -> index` for one `count`/`id` registry pair, read once (C1).
  #indicesOf(countExport, idExport) {
    const indices = new Map();
    const total = this.#exports[countExport]();
    for (let i = 0; i < total; i += 1) {
      const bytes = this.framed(this.#exports[idExport](i));
      indices.set(new TextDecoder("utf-8", { fatal: true }).decode(bytes), i);
    }
    return indices;
  }

  layouts() {
    if (this.#layouts === null) this.#layouts = this.#indicesOf("gm_layout_count", "gm_layout_id");
    return this.#layouts;
  }

  analyses() {
    if (this.#analyses === null) {
      this.#analyses = this.#indicesOf("gm_analysis_count", "gm_analysis_id");
    }
    return this.#analyses;
  }

  posts() {
    if (this.#posts === null) this.#posts = this.#indicesOf("gm_post_count", "gm_post_id");
    return this.#posts;
  }

  /// The registry index of `name`, or could-not-run. `gm_run`'s `layout_id` argument is
  /// this index, so a name the module does not carry is refused here rather than run as
  /// whatever happens to sit at that index.
  layoutIndex(name) {
    const index = this.layouts().get(name);
    if (index === undefined) refuse(`no registered layout named ${name}`);
    return index;
  }

  /// gm_seed_ingest -> gm_alloc -> gm_build: the seed's model on a fresh handle, which
  /// this arm owns and must `gm_release`. The layout, analysis and POST paths all start
  /// here, so a seed's model is built one way and a build refusal is reported once.
  #buildHandle(seed) {
    const ingest = this.framed(this.#exports.gm_seed_ingest(seed));
    const ptr = this.#exports.gm_alloc(ingest.length);
    if (ptr === 0) refuse(`gm_alloc refused ${ingest.length} bytes (seed ${seed})`);
    new Uint8Array(this.#exports.memory.buffer, ptr, ingest.length).set(ingest);
    const handle = this.#exports.gm_build(ptr, ingest.length);
    this.#exports.gm_free(ptr, ingest.length);
    if (handle === 0) {
      refuse(`gm_build refused (seed ${seed}, gm_last_error ${this.#exports.gm_last_error()})`);
    }
    return handle;
  }

  /// `gm_run(handle, layout)` over the seed's model, or a refusal.
  #runLayout(handle, seed, layout) {
    const ok = this.#exports.gm_run(handle, this.layoutIndex(layout), 0, 0);
    if (ok !== 1) {
      refuse(`gm_run refused (seed ${seed}, gm_last_error ${this.#exports.gm_last_error()})`);
    }
  }

  /// C20: the *real* ABI — gm_seed_ingest's provisional-ingest text through
  /// gm_alloc/gm_build/gm_run/gm_snapshot_bytes — for the gate's seed model, so its hash
  /// can be asserted equal to the retained gm_layout_grid shim's (both are the binary face
  /// of the same model run through the same layout).
  abiSnapshotBytes(seed, layout) {
    const handle = this.#buildHandle(seed);
    this.#runLayout(handle, seed, layout);
    const bytes = this.framed(this.#exports.gm_snapshot_bytes(handle));
    this.#exports.gm_release(handle);
    return bytes;
  }

  /// The canonical JSON face of analysis `index` over the seed's topology, through the
  /// real ABI: gm_build -> gm_analysis_run. No layout is run — every analysis is a
  /// function of the topology alone — so the framed UTF-8 `gm_analysis_run` publishes is
  /// exactly what the native arm's `analysis::to_json` writes.
  abiAnalysisBytes(seed, index) {
    const handle = this.#buildHandle(seed);
    const bytes = this.framed(this.#exports.gm_analysis_run(handle, index));
    this.#exports.gm_release(handle);
    return bytes;
  }

  /// The snapshot POST capability `index` produces over the seed's model, through the real
  /// ABI: gm_build -> gm_run(SHIM_LAYOUT) -> gm_post_run -> gm_snapshot_bytes. The grid
  /// is run first because a POST pass reads positions, and it is the same layout the
  /// transport stage states; the native arm runs the same pass over the same geometry.
  abiPostBytes(seed, index) {
    const handle = this.#buildHandle(seed);
    this.#runLayout(handle, seed, SHIM_LAYOUT);
    const ok = this.#exports.gm_post_run(handle, index);
    if (ok !== 1) {
      refuse(`gm_post_run refused (seed ${seed}, gm_last_error ${this.#exports.gm_last_error()})`);
    }
    const bytes = this.framed(this.#exports.gm_snapshot_bytes(handle));
    this.#exports.gm_release(handle);
    return bytes;
  }

  /// The probe buffer's hex face, or a refusal naming the feature that carries it.
  probeHex() {
    if (typeof this.#exports.gm_probe !== "function") {
      refuse("no gm_probe export: build graph-wasm with --features probe");
    }
    return Buffer.from(this.framed(this.#exports.gm_probe())).toString("hex");
  }

  /// The stages hashable without consulting a registry: the two retained shims, unchanged
  /// since Phase 2/3 so their already-green gate keeps hashing the same bytes, and the
  /// transport, which is the real ABI over the shim's layout. Every other stage is a
  /// registered layout, analysis or POST id, hashed through the real ABI (`bytesFor`).
  ///
  /// A **null prototype**, so a lookup cannot fall through to `Object.prototype`: an
  /// object literal would answer `table["toString"]` with `Object.prototype.toString`, and
  /// the arm would then *hash* that member's return value under a stage nobody ran — a
  /// digest of the string "[object Undefined]" and exit 0 for a stage that does not exist.
  /// An unknown stage is refused by name instead (exit 2, "could not run"), never hashed as
  /// something else; `hashgate/tests/stages/arm.rs` holds both directions.
  get stageBytes() {
    if (this.#stageBytes === null) {
      this.#stageBytes = Object.assign(Object.create(null), {
        topology: (seed) => this.framed(this.#exports.gm_topology(seed)),
        [SHIM_LAYOUT]: (seed) => this.framed(this.#exports.gm_layout_grid(seed)),
        [TRANSPORT_STAGE]: (seed) => this.abiSnapshotBytes(seed, SHIM_LAYOUT),
      });
    }
    return this.#stageBytes;
  }

  /// How to hash `stage` for one seed, resolved by name against the module's own registries
  /// (C1): the retained shims, then a registered layout, analysis or POST capability, each
  /// through the real ABI. A name in none of them is refused here rather than hashed as
  /// whatever sits at some index, and the null-prototype table means `toString` cannot
  /// fall through to `Object.prototype` either.
  bytesFor(stage) {
    const table = this.stageBytes;
    if (stage in table) return table[stage];
    if (this.layouts().has(stage)) return (seed) => this.abiSnapshotBytes(seed, stage);
    const analysis = this.analyses().get(stage);
    if (analysis !== undefined) return (seed) => this.abiAnalysisBytes(seed, analysis);
    const post = this.posts().get(stage);
    if (post !== undefined) return (seed) => this.abiPostBytes(seed, post);
    return refuse(`unknown stage ${stage}: not a registered layout, analysis and post capability`);
  }

  /// The stage ids this arm can hash, in the gate's own order: the topology, every
  /// registered layout, every analysis, every POST capability, then the transport. The
  /// stage list is the module's, not a literal list here (C1).
  stageList() {
    return [
      "topology",
      ...this.layouts().keys(),
      ...this.analyses().keys(),
      ...this.posts().keys(),
      TRANSPORT_STAGE,
    ];
  }
}
