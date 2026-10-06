/** The label policy each choice of the dock's Labels control stands for. */
import { DEFAULT_POLICY, type LabelPolicy } from "../../../graph-render/src/labels.ts";
import type { Appearance } from "../state/settings.ts";

export const LABEL_POLICIES: Readonly<Record<Appearance["labels"], LabelPolicy>> = {
  auto: DEFAULT_POLICY,
  more: { threshold: 0.45, budget: 400 },
  none: { threshold: DEFAULT_POLICY.threshold, budget: 0 },
};

export function policyOf(appearance: Appearance): LabelPolicy {
  return { ...LABEL_POLICIES[appearance.labels], fade: appearance.textFade };
}
