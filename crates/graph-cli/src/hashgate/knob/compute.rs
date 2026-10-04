//! Why the two compute-tier controls and the live session's own exist.
//!
//! Split out of `knob.rs` by the house's 300-line limit, and because these three are the only
//! controls whose claim is about a **merge** rather than a parameter or a model's size — the
//! prose is three long arguments that do not fit beside a forty-four-arm enum without pushing
//! it over, and an arm doc that is a paragraph of arithmetic is a paragraph nobody reads. The
//! arms stay in `knob.rs` beside the parameters they sit among; only the reasoning moved here,
//! and each of the three arms points at the heading below, so the link is one click rather than
//! a guess.
//!
//! What the three have in common, and what separates them: all three corrupt something the
//! threaded arms **recompute**, so a threaded arm that ignored it would agree with a mutated one
//! and "4-way equal" would be a statement about nothing. Two split a gathered merge — the three
//! Barnes-Hut passes' own, and the closed-form point layouts' single `coords` merge. The third
//! moves a parameter only the *live* force session has.

// ---------------------------------------------------------------------------------------------
// `GM_MUTATE_SPLIT_SUM` — `Knob::SplitSum`.
//
// Phase 11's own control, and the one the phase prompt names: it makes a gathered pass's merge
// read a *neighbouring* node's delta — the shape a wrong partition of the outputs would take —
// so the threaded arms must diverge from the scalar one. It is the control that proves the
// threaded arms are actually reading their own results: a threaded arm that ignored the merge
// entirely would agree with a mutated one.
//
// The variable takes the **pass** whose merge is split (`charge`, `collide`, `link`), because
// each kernel needs its own control to be shown to be compared: a knob that only ever split the
// charge merge would leave the other two kernels' equality resting on nothing. `1`/`true` means
// all three, `0`/`false` none. It is parsed rather than treated as a presence flag, so `=0` is
// the honest run and a typo (`=maybe`) an error rather than a silent mutation.
//
// The perturbation lives in that pass's merge (`barnes_hut::charge`, `barnes_hut::collide`,
// `barnes_hut::link`), and the setting it reads is carried on the `Setting` so a knob cannot
// change behaviour without being declared on the enum — the same discipline every other knob
// obeys.
// ---------------------------------------------------------------------------------------------

// ---------------------------------------------------------------------------------------------
// `GM_MUTATE_SPLIT_RESCALE` — `Knob::SplitRescale`.
//
// The closed-form point layouts' sibling of the above, and the control that makes the *other*
// half of Phase 11 provable: it makes the `rescale_layout` centroid merge read the **next**
// node's term into this node's — the shape a wrong partition of the outputs would take — so the
// threaded arms of `layout.grid`, `layout.circular.ring` and `layout.spiral` must diverge from
// the scalar one, and the arms of every other stage must not.
//
// **It reaches the merge, not the gather.** A knob that perturbed a layout's own arithmetic
// would move the *scalar* arm too and so would prove only that the stage is hashed; this one
// exists to prove the threaded arm **recomputed** the merge. A threaded arm that reused the
// scalar column would agree with a mutated one, and "10-way equal" would be a statement about
// nothing.
//
// A `bool` and not a pass-name, because there is exactly one merge to name — the three
// Barnes-Hut passes each needed their own variant so a row could prove a *particular* kernel
// was compared, and one merge cannot be told apart from itself. It is a compiled-in parameter,
// never a `cfg` and never an environment read, for the reason above: graph-core reads no clock,
// no environment and no hardware, and the host supplies even the mutation.
//
// **The grid's own control answers a different question.** `Knob::GridSpacing` is a *pass*
// control: it moves a real parameter, so it moves the scalar arm and every threaded arm alike,
// and what it proves is that `layout.grid` is hashed and compared at all. This one is the *fail*
// control for the merge the three layouts share, and it moves only the threaded arms. Both are
// kept: one question each, neither standing in for the other.
// ---------------------------------------------------------------------------------------------

// ---------------------------------------------------------------------------------------------
// `GM_MUTATE_FORCE_SESSION_GRAVITY` — `Knob::ForceSessionGravity`.
//
// Its own control, and the only one that reaches the live session: no other variable in this
// list touches `LiveParams`, because `LiveParams` has no other user on this side — the frozen
// stage runs `from_frozen`, which is a parameter set `ForceParams` holds and
// `Knob::ForceTheta` already reaches through. `gravity` is on top of that the one parameter the
// **frozen** set does not have at all (`live_params.rs`), so a control that moves it cannot
// possibly move `layout.force.barnes_hut` and take another stage with it.
//
// It reaches the force gate rather than this gate: the wasm arm builds the seed's model from
// `gm_seed_ingest`, whose document is fixed, so the only perturbation a cross-target comparison
// here can see is one in the *parameters* — and this is that one. A non-zero value pulls every
// node toward the origin, which moves every position in the pair of columns the gate hashes, on
// every seed, from the first tick.
//
// Parsed, not treated as a flag: `=0` must be the honest run and a typo (`=maybe`) an error
// rather than a silent no-op — the same rule every other parameter knob obeys, for the same
// reason.
// ---------------------------------------------------------------------------------------------

// ---------------------------------------------------------------------------------------------
// `GM_MUTATE_DROP_DELTA` — `Knob::DropDelta`.
//
// The second control to reach `force-gate` rather than this gate, and the first that reaches
// the *stream* stage rather than the session's parameters: the native arm skips one batch
// outright — no `extend`, no `grow` — while the wasm arm, which reads no environment variable,
// still receives it. So the two sessions carry different node counts from that batch on, and
// every later digest must differ.
//
// **Why the control drops input rather than moving a parameter.** A gravity that merely
// differs would also work, and `Knob::ForceSessionGravity` already proves the tick is
// compared. This one is for the growth path specifically: `Topology::extend` and
// `ForceSession::grow` are the two exports this slice added, and a control that perturbs
// nothing *about growth* would leave both of them untested — the gate could agree on every
// batch because it never disagreed about what to grow. Dropping batch `k` puts the native
// session one batch behind forever, which is the failure the stream stage exists to catch.
//
// It bites from the batch it names onward, never before: batches `0..k` are still processed
// identically on both arms, so a gate that reported "diverged at batch 0" would be pointing
// at the wrong line. `=0` is refused at the parse rather than at the no-op check, because
// batch 0 is the initial graph — the one line that is not a delta — and skipping it would
// drop nothing while still recording this run as the exercised control.
// ---------------------------------------------------------------------------------------------
