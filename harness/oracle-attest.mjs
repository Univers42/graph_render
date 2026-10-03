// Attestation for the arms that measure bytes no producer in this tree stamps with a
// digest: the tree fingerprint (graph-cli's `evidence.rs` shape, restated here so no arm
// imports another arm's module) and a small seal under <gates>/.
//
// The seal buys less than a producer stamp and says so. What it does buy is real: once a
// *passing* run has recorded `{fingerprint, sha256}` for a tree, a later run over that same
// tree that measures different bytes is refused instead of passed. A dump, or a fixture set
// whose manifest was re-sealed by hand, can no longer be reported as a pass after a green
// run has seen different bytes on the same tree.
//
// Ponytail: the first run over an unattested measurement has nothing to compare against and
// passes, and a tree edit re-attests (the seal is keyed by the fingerprint, so new bytes on
// a new tree are a new epoch, not a tamper). Only the producer closes that: stamp the
// fingerprint and a digest of the measured bytes into the dump or manifest itself.

import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, readdirSync, statSync, writeFileSync } from "node:fs";
import { dirname, join, relative, sep } from "node:path";

export const sha256Hex = (data) => createHash("sha256").update(data).digest("hex");

/**
 * The tree fingerprint over `entries` (paths relative to `root`), computed exactly as
 * graph-cli's `evidence.rs` computes it: every file under each entry, sorted by its
 * `/`-joined relative path compared as bytes, as `path\0sha256(bytes)\n`.
 */
export function treeFingerprint(root, entries) {
  const files = [];
  const walk = (path) => {
    if (statSync(path).isFile()) files.push(relative(root, path).split(sep).join("/"));
    else for (const child of readdirSync(path).sort()) walk(join(path, child));
  };
  for (const entry of entries) walk(join(root, entry));
  files.sort((a, b) => Buffer.compare(Buffer.from(a), Buffer.from(b)));
  return sha256Hex(files.map((f) => `${f}\0${sha256Hex(readFileSync(join(root, f)))}\n`).join(""));
}

/** The seal's own path beside a gate record: `<gates>/<gate>.seal.json`. */
export const sealPathFor = (gates, gate) => join(gates, `${gate}.seal.json`);

/**
 * Refuses a measurement whose bytes changed under an unchanged tree since the last passing
 * run recorded them, then records this run's. Returns what it did, for the arm's record.
 *
 * @param sealPath where the seal lives; @param gate the name both messages name;
 * @param fingerprint the tree the measurement claims to come from;
 * @param sha256 digest of the measured bytes; @param pass whether this run passed.
 */
/** The seal at `sealPath`, or `null` when this is the first sighting. */
export const readSeal = (sealPath) => (existsSync(sealPath) ? JSON.parse(readFileSync(sealPath, "utf8")) : null);

/** Writes the seal for this run's measurement. */
export function writeSeal(sealPath, seal) {
  mkdirSync(dirname(sealPath), { recursive: true });
  writeFileSync(sealPath, `${JSON.stringify(seal, null, 2)}\n`);
}

/**
 * Throws when the bytes about to be measured differ from those a *passing* run recorded for
 * the same tree. Call this before measuring, so a tampered or re-sealed input is refused
 * rather than measured.
 */
export function refuseChangedBytes({ sealPath, gate, fingerprint, sha256 }) {
  const previous = readSeal(sealPath);
  if (previous?.pass === true && previous.fingerprint === fingerprint && previous.sha256 !== sha256) {
    throw new Error(`${gate}: the measured bytes changed under an unchanged tree since the last passing run (${previous.sha256.slice(0, 12)} -> ${sha256.slice(0, 12)}): the dump or fixture set was edited, or re-sealed by hand`);
  }
  return { fingerprint, sha256, attested: previous !== null };
}

/**
 * Refuses (via `refuseChangedBytes`) and then records, for the arm that knows its verdict
 * before it writes anything. Returns what it did, for the arm's record.
 *
 * @param sealPath where the seal lives; @param gate the name both messages name;
 * @param fingerprint the tree the measurement claims to come from;
 * @param sha256 digest of the measured bytes; @param pass whether this run passed.
 */
export function attest(seal) {
  const seen = refuseChangedBytes(seal);
  const previous = readSeal(seal.sealPath);
  writeSeal(seal.sealPath, { gate: seal.gate, fingerprint: seal.fingerprint, sha256: seal.sha256, pass: seal.pass });
  return { ...seen, reattested: previous !== null && previous.fingerprint !== seal.fingerprint };
}