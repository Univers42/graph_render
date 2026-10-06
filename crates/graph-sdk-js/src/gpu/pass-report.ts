/**
 * `pass-report.ts` — the verdict of one non-charge pass (link, collide) against its fixture
 * column, the shape the harness prints. Charge keeps its own `ChargeReport`, whose deposit and
 * bounds checks no other pass has.
 */

/** A pass the per-pass probe runs besides charge. */
export type PassKind = "link" | "collide";

/** One case's verdict. `pass` is false when any guard, ceiling or exactness check failed. */
export interface PassReport {
  readonly kind: PassKind;
  readonly n: number;
  readonly state: 0 | 1;
  readonly rmsAbs: number;
  readonly rmsRef: number;
  readonly rmsRel: number;
  readonly maxAbs: number;
  /** Two runs on one device, byte for byte. */
  readonly repeatEqual: boolean;
  /** The pass's own exactness checks by name, each true when it held (collide: `order`). */
  readonly exact: Readonly<Record<string, boolean>>;
  readonly pass: boolean;
  readonly failures: readonly string[];
  /** `vendor/architecture` as the browser reported them. */
  readonly marks: string;
  readonly fallback: boolean;
}
