/**
 * The force knobs as a saved member of the settings. A document written before the knobs
 * were saved has no `forces`, and one written before a knob existed has no value for it:
 * either way the missing value is the motor's default, so every older document still loads.
 * A value that is there is checked against the panel's own limits, like every other member.
 */
import { DEFAULT_KNOBS, type ForceKnobs, KNOB_LIMITS, type KnobName } from "../motor/live.ts";
import { type Fields, fieldsOf, numberOf } from "./read.ts";

/** Members in one fixed order, so two equal documents are equal as text. */
export function forcesOf(knobs: ForceKnobs): ForceKnobs {
  return Object.freeze({
    gravity: knobs.gravity, charge: knobs.charge, linkStrengthScale: knobs.linkStrengthScale,
    linkDistance: knobs.linkDistance, collideRadius: knobs.collideRadius, velocityDecay: knobs.velocityDecay,
    alphaDecay: knobs.alphaDecay, distanceMax: knobs.distanceMax, theta: knobs.theta,
  });
}

export function sameKnobs(a: ForceKnobs, b: ForceKnobs): boolean {
  return JSON.stringify(forcesOf(a)) === JSON.stringify(forcesOf(b));
}

function knobOf(fields: Fields, at: string, name: KnobName): number {
  if (fields[name] === undefined) return DEFAULT_KNOBS[name];
  return numberOf(fields, at, name, { ...KNOB_LIMITS[name], whole: false });
}

export function readForces(value: unknown, at = "settings.forces"): ForceKnobs {
  if (value === undefined) return forcesOf(DEFAULT_KNOBS);
  const fields = fieldsOf(value, at, Object.keys(KNOB_LIMITS));
  const read = (name: KnobName): number => knobOf(fields, at, name);
  return forcesOf({
    gravity: read("gravity"), charge: read("charge"), linkStrengthScale: read("linkStrengthScale"),
    linkDistance: read("linkDistance"), collideRadius: read("collideRadius"), velocityDecay: read("velocityDecay"),
    alphaDecay: read("alphaDecay"), distanceMax: read("distanceMax"), theta: read("theta"),
  });
}
