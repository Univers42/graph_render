/* ************************************************************************** */
/*                                                                            */
/*                                                        :::      ::::::::   */
/*   graph-engine-ids.test.ts                           :+:      :+:    :+:   */
/*                                                    +:+ +:+         +:+     */
/*   By: dlesieur <dlesieur@student.42.fr>          +#+  +:+       +#+        */
/*                                                +#+#+#+#+#+   +#+           */
/*   Created: 2026/06/08 12:00:00 by dlesieur          #+#    #+#             */
/*   Updated: 2026/06/08 12:00:00 by dlesieur         ###   ########.fr       */
/*                                                                            */
/* ************************************************************************** */

import assert from "node:assert/strict";
import test from "node:test";

import {
  makeNoteNodeId,
  makeRecordNodeId,
  makeTagNodeId,
  parseNodeId,
} from "../src/core/model/ids.ts";
import { edgeKindFromType } from "../src/core/model/edgeKind.ts";

test("parseNodeId: the read half round-trips the write half", () => {
  const id = makeRecordNodeId("db", "people", "rec-42");
  assert.deepEqual(parseNodeId(id), { source: "db", databaseId: "people", recordId: "rec-42" });
  // recordId may itself contain ':' — everything after the 2nd segment must
  // rejoin, otherwise a composite primary key silently truncates and the id
  // addresses a different record than it was built for.
  assert.deepEqual(parseNodeId(makeRecordNodeId("db", "people", "a:b:c")), {
    source: "db", databaseId: "people", recordId: "a:b:c",
  });
  // Degenerate input must not throw. "bare" has no second segment, so databaseId
  // and recordId are empty rather than absent.
  assert.deepEqual(parseNodeId("bare"), { source: "bare", databaseId: "", recordId: "" });
});

test("parseNodeId: refuses non-record ids instead of inventing coordinates", () => {
  // makeNoteNodeId / makeTagNodeId manufacture these two prefixes, and the
  // previous non-nullable return type answered them with a populated-looking
  // RecordRef whose fields meant something else entirely.
  assert.equal(parseNodeId(makeNoteNodeId("n1")), null);
  assert.equal(parseNodeId(makeTagNodeId("vintage")), null);
  assert.equal(parseNodeId("tag:"), null);
  assert.equal(parseNodeId("note:"), null);
  // A real record id is unaffected.
  assert.notEqual(parseNodeId(makeRecordNodeId("db", "people", "r")), null);
});

test("edgeKindFromType: wire type -> internal kind, unknown falls back to relation", () => {
  assert.equal(edgeKindFromType("parent"), "hierarchy");
  assert.equal(edgeKindFromType("parent_of"), "hierarchy");
  assert.equal(edgeKindFromType("child_of"), "hierarchy");
  assert.equal(edgeKindFromType("page_hierarchy"), "hierarchy");
  assert.equal(edgeKindFromType("note_link"), "note_link");
  assert.equal(edgeKindFromType("links_to"), "note_link");
  assert.equal(edgeKindFromType("note_of"), "note_of");
  assert.equal(edgeKindFromType("annotates"), "note_of");
  assert.equal(edgeKindFromType("tagged"), "tag");
  assert.equal(edgeKindFromType("tag"), "tag");
  assert.equal(edgeKindFromType("TAGGED"), "tag", "case-insensitive on contract literals");
  // NOT tags any more. These used to be asserted as "tag" on the strength of a
  // substring match, which is what let "vintage" and "heritage" through as well.
  // The contract's tag literals are exactly `tagged` and `tag`; anything else is
  // a user-named type and lands on the neutral default.
  assert.equal(edgeKindFromType("tagged_by"), "relation");
  assert.equal(edgeKindFromType("Tags"), "relation");
  assert.equal(edgeKindFromType("tags"), "relation");
  // Everything unrecognized is a plain structural relation — this is the
  // default every relation-property edge lands on, so it must be the fallback.
  assert.equal(edgeKindFromType("assignee"), "relation");
  // Branch order is observable and worth pinning: the hierarchy test is an exact
  // match on three literals plus a "hierarchy" substring, NOT a "parent"
  // substring. Documented so a future "widening" is a deliberate change.
  assert.equal(edgeKindFromType("parent_hierarchy"), "hierarchy");
  assert.equal(edgeKindFromType("project"), "relation");

  // The tag branch must NOT substring-match. The wire `type` is free text chosen
  // by whoever built the database, and these are all ordinary field names:
  // he-ri-TAG-e, advanTAG-e, monTAG-e, fronTAG-e, cotTAG-e, sTAG-e. A previous
  // version used `includes("tag")` and classified every one of them as a tag
  // edge, which repaints them with the tag colour, the tag geometry tier, and
  // the tag filter predicate — silently, with no error anywhere.
  for (const word of ["vintage", "heritage", "advantage", "montage", "frontage", "cottage", "stage"]) {
    assert.equal(edgeKindFromType(word), "relation", `"${word}" must not classify as a tag edge`);
  }
  // Same for the other substring branches, whose markers are engine-generated
  // and so genuinely unambiguous.
  assert.equal(edgeKindFromType("my_page_hierarchy"), "hierarchy");
  assert.equal(edgeKindFromType(undefined), "relation");
  assert.equal(edgeKindFromType(""), "relation");
});
