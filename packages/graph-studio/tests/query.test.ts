import assert from "node:assert/strict";
import { test } from "node:test";

import {
  type Query,
  type QueryField,
  QueryRefusal,
  parseQuery,
  printQuery,
} from "../src/console/parse.ts";

const text = (word: string): Query => ({ kind: "text", text: word });
const field = (name: QueryField, op: string, value: string): Query =>
  ({ kind: "field", field: name, op, value });

test("an empty query is the whole graph", () => {
  assert.deepEqual(parseQuery(""), { kind: "all" });
  assert.deepEqual(parseQuery("   "), { kind: "all" });
});

test("a bare or quoted word with no field is free text", () => {
  assert.deepEqual(parseQuery("a"), text("a"));
  assert.deepEqual(parseQuery('"two words"'), text("two words"));
  assert.deepEqual(parseQuery("'two words'"), text("two words"));
  assert.deepEqual(parseQuery('""'), text(""));
  assert.deepEqual(parseQuery('"two:words"'), text("two:words"));
});

test("a quoted value is a value: an operator inside quotes is content, and `''` is a value", () => {
  assert.deepEqual(parseQuery('kind:""'), field("kind", "", ""));
  assert.deepEqual(parseQuery('kind:"=x"'), field("kind", "", "=x"));
  assert.deepEqual(parseQuery('kind:">x"'), field("kind", "", ">x"));
  assert.deepEqual(parseQuery('id:>"3"'), field("id", ">", "3"));
  assert.deepEqual(parseQuery("tag: ='#x'"), field("tag", "=", "#x"));
  assert.throws(() => parseQuery('degree:">3"'), QueryRefusal);
});

test("a field takes a value, with `=` implied when no operator is written", () => {
  assert.deepEqual(parseQuery("kind:note"), field("kind", "", "note"));
  assert.deepEqual(parseQuery("id:42"), field("id", "", "42"));
  assert.deepEqual(parseQuery("db:blog"), field("db", "", "blog"));
  assert.deepEqual(parseQuery("path:src/a.ts"), field("path", "", "src/a.ts"));
  assert.deepEqual(parseQuery("kind:!=note"), field("kind", "!=", "note"));
  assert.deepEqual(parseQuery('kind:"two words"'), field("kind", "", "two words"));
  assert.deepEqual(parseQuery('kind: "two words"'), field("kind", "", "two words"));
  assert.deepEqual(parseQuery("kind:note AND db:blog"), {
    kind: "and",
    of: [field("kind", "", "note"), field("db", "", "blog")],
  });
});

test("a tag keeps the `#`, whether or not it is written", () => {
  assert.deepEqual(parseQuery("tag:#x"), field("tag", "", "#x"));
  assert.deepEqual(parseQuery("tag:x"), field("tag", "", "x"));
  assert.deepEqual(parseQuery('tag:"#two words"'), field("tag", "", "#two words"));
});

test("`version:` is numeric, like `degree:`, and takes a fraction where a degree cannot", () => {
  assert.deepEqual(parseQuery("version:>=1700000000"), field("version", ">=", "1700000000"));
  assert.deepEqual(parseQuery("version:>1.5"), field("version", ">", "1.5"));
  assert.deepEqual(parseQuery("version:=1700000000"), field("version", "=", "1700000000"));
});

test("`group:` is a name: no operator, and `=` written or implied", () => {
  assert.deepEqual(parseQuery("group:Ana"), field("group", "", "Ana"));
  assert.deepEqual(parseQuery("group:=Ana"), field("group", "=", "Ana"));
  assert.deepEqual(parseQuery("GROUP:Ana"), field("group", "", "Ana"));
  assert.deepEqual(parseQuery('group:"Ana B"'), field("group", "", "Ana B"));
});

test("`degree:` needs its operator, and `version:` does too; no name field does", () => {
  assert.deepEqual(parseQuery("degree:>3"), field("degree", ">", "3"));
  assert.deepEqual(parseQuery("degree:<3"), field("degree", "<", "3"));
  assert.deepEqual(parseQuery("degree:>=3"), field("degree", ">=", "3"));
  assert.deepEqual(parseQuery("degree:<=2"), field("degree", "<=", "2"));
  assert.deepEqual(parseQuery("degree:=3"), field("degree", "=", "3"));
  assert.deepEqual(parseQuery("degree: >3"), field("degree", ">", "3"));
  assert.deepEqual(parseQuery("degree:> 3"), field("degree", ">", "3"));
});

test("keywords and field names are read in any case, the value is left alone", () => {
  assert.deepEqual(parseQuery("KIND:Note"), field("kind", "", "Note"));
  assert.deepEqual(parseQuery("a and b"), { kind: "and", of: [text("a"), text("b")] });
  assert.deepEqual(parseQuery("a And b"), { kind: "and", of: [text("a"), text("b")] });
  assert.deepEqual(parseQuery("a oR b"), { kind: "or", of: [text("a"), text("b")] });
  assert.deepEqual(parseQuery("nOt a"), { kind: "not", of: text("a") });
  assert.deepEqual(parseQuery("DEGREE:>3"), field("degree", ">", "3"));
});

test("NOT binds tighter than AND, and AND tighter than OR", () => {
  assert.deepEqual(parseQuery("a OR b AND c"), { kind: "or", of: [text("a"), { kind: "and", of: [text("b"), text("c")] }] });
  assert.deepEqual(parseQuery("a AND b OR c"), { kind: "or", of: [{ kind: "and", of: [text("a"), text("b")] }, text("c")] });
  assert.deepEqual(parseQuery("NOT a AND b"), { kind: "and", of: [{ kind: "not", of: text("a") }, text("b")] });
  assert.deepEqual(parseQuery("NOT a OR b"), { kind: "or", of: [{ kind: "not", of: text("a") }, text("b")] });
  assert.deepEqual(parseQuery("a AND NOT b OR c"), {
    kind: "or",
    of: [{ kind: "and", of: [text("a"), { kind: "not", of: text("b") }] }, text("c")],
  });
});

test("AND and OR take any number of terms, and a group of one is the term itself", () => {
  assert.deepEqual(parseQuery("a AND b AND c"), { kind: "and", of: [text("a"), text("b"), text("c")] });
  assert.deepEqual(parseQuery("a OR b OR c"), { kind: "or", of: [text("a"), text("b"), text("c")] });
  assert.deepEqual(parseQuery("(a)"), text("a"));
  assert.deepEqual(parseQuery("((a AND b))"), { kind: "and", of: [text("a"), text("b")] });
  assert.deepEqual(parseQuery("a AND (b AND c)"), { kind: "and", of: [text("a"), text("b"), text("c")] });
  assert.deepEqual(parseQuery("a OR (b OR c)"), { kind: "or", of: [text("a"), text("b"), text("c")] });
});

test("parentheses group, and NOT reaches into a group", () => {
  assert.deepEqual(parseQuery("(a OR b) AND c"), { kind: "and", of: [{ kind: "or", of: [text("a"), text("b")] }, text("c")] });
  assert.deepEqual(parseQuery("a AND (b OR c)"), { kind: "and", of: [text("a"), { kind: "or", of: [text("b"), text("c")] }] });
  assert.deepEqual(parseQuery("NOT (a AND b)"), { kind: "not", of: { kind: "and", of: [text("a"), text("b")] } });
  assert.deepEqual(parseQuery("NOT (a OR b)"), { kind: "not", of: { kind: "or", of: [text("a"), text("b")] } });
  assert.deepEqual(parseQuery("NOT NOT a"), { kind: "not", of: { kind: "not", of: text("a") } });
  assert.deepEqual(parseQuery("(NOT a) AND b"), { kind: "and", of: [{ kind: "not", of: text("a") }, text("b")] });
  assert.deepEqual(parseQuery("a AND (NOT b)"), { kind: "and", of: [text("a"), { kind: "not", of: text("b") }] });
  assert.deepEqual(parseQuery("(a AND b) OR (c AND d)"), {
    kind: "or",
    of: [
      { kind: "and", of: [text("a"), text("b")] },
      { kind: "and", of: [text("c"), text("d")] },
    ],
  });
});

const REFUSED: readonly (readonly [string, string, number, string])[] = [
  ["an unclosed double quote", 'a "bc', 3, '3: a " quote is never closed'],
  ["an unclosed single quote", "a 'bc", 3, "3: a ' quote is never closed"],
  ["a close with no open", "a)", 2, "2: a `)` has no `(`"],
  ["a close with no open, after a group", "(a))", 4, "4: a `)` has no `(`"],
  ["an open that is never closed", "(a", 1, "1: a `(` is never closed"],
  ["a word after a complete expression", "a b", 3, "3: a word follows the end of the query"],
  ["a word after a complete field", "kind:note db:blog", 11, "11: a word follows the end of the query"],
  ["a missing value", "kind:", 6, "6: a value is missing after `kind:`"],
  ["a missing value before a keyword", "a AND kind: OR b", 12, "12: a value is missing after `kind:`"],
  ["a missing value at the end of a group", "(kind:)", 7, "7: a value is missing after `kind:`"],
  ["an unknown field", "colour:red", 1, "1: `colour` is not a field"],
  ["an unknown field after a word", "a AND colour:red", 7, "7: `colour` is not a field"],
  ["a colon in a bare word is read as a field", "12:30", 1, "1: `12` is not a field"],
  ["a degree with no operator", "degree:3", 8, "8: `degree:` needs an operator: one of > < >= <= ="],
  ["a degree with an unknown operator", "degree:!=3", 8, "8: `degree:` does not know the operator `!=`"],
  ["a degree with a non-integer", "degree:>x", 9, "9: `degree:` needs a whole number after `>`"],
  ["a degree with a negative", "degree:>-1", 9, "9: `degree:` needs a whole number after `>`"],
  ["a degree with an operator inside quotes", 'degree:">3"', 8, "8: `degree:` needs an operator: one of > < >= <= ="],
  ["a version with no operator", "version:5", 9, "9: `version:` needs an operator: one of > < >= <= ="],
  ["a version that is not a number either", "version:abc", 9, "9: `version:` needs an operator: one of > < >= <= ="],
  ["a version with an unknown operator", "version:!=5", 9, "9: `version:` does not know the operator `!=`"],
  ["a version with an operator inside quotes", 'version:">5"', 9, "9: `version:` needs an operator: one of > < >= <= ="],
  ["a version with a value that is not a number", "version:>abc", 10, "10: `version:` needs a number after `>`"],
  ["a version with a trailing point", "version:>1.", 10, "10: `version:` needs a number after `>`"],
  ["a version with an empty value", 'version:>""', 10, "10: `version:` needs a number after `>`"],
  ["a dangling AND", "a AND", 3, "3: `AND` has nothing to join"],
  ["a dangling OR", "a OR", 3, "3: `OR` has nothing to join"],
  ["a dangling AND before a close", "a AND )", 3, "3: `AND` has nothing to join"],
  ["a dangling AND before an OR", "a AND OR b", 3, "3: `AND` has nothing to join"],
  ["a dangling NOT", "a AND NOT", 7, "7: `NOT` has nothing to negate"],
  ["a NOT with nothing at all", "NOT", 1, "1: `NOT` has nothing to negate"],
  ["a leading AND", "AND a", 1, "1: the query starts with `AND`"],
  ["a leading OR", "OR a", 1, "1: the query starts with `OR`"],
  ["a keyword right after NOT", "NOT AND a", 5, "5: `AND` cannot follow `NOT`"],
  ["an open followed by a word, no close", "(a b", 1, "1: a `(` is never closed"],
  ["an empty group", "a AND ( )", 9, "9: the group between `(` and `)` is empty"],
  ["an empty group of its own", "( )", 3, "3: the group between `(` and `)` is empty"],
];

for (const [name, text_, column, message] of REFUSED) {
  test(`${name} is refused, with the column it gave up at`, () => {
    assert.throws(
      () => parseQuery(text_),
      (error: unknown) => {
        if (!(error instanceof QueryRefusal)) return false;
        assert.equal(error.message, message);
        assert.equal(error.column, column);
        return true;
      },
    );
  });
}

const PRINTED: readonly (readonly [string, Query, string])[] = [
  ["the whole graph", { kind: "all" }, ""],
  ["a word", text("a"), "a"],
  ["a word with a space", text("two words"), '"two words"'],
  ["an empty word", text(""), '""'],
  ["a word that is a keyword", text("and"), '"and"'],
  ["a word that is a keyword, shouted", text("NOT"), '"NOT"'],
  ["a word with a colon", text("two:words"), '"two:words"'],
  ["a word with a paren", text("a)b"), '"a)b"'],
  ["a word with a hash", text("a#b"), '"a#b"'],
  ["a word with a quote", text('say "hi"'), '"say \\"hi\\""'],
  ["a word with a backslash, which a bare word reads literally", text("a\\b"), "a\\b"],
  ["a plain field", field("kind", "", "note"), "kind:note"],
  ["a field with an operator", field("kind", "!=", "note"), "kind:!=note"],
  ["a tag", field("tag", "", "#x"), "tag:#x"],
  ["a bare tag", field("tag", "", "x"), "tag:x"],
  ["a tag with a space", field("tag", "", "#two words"), 'tag:"#two words"'],
  ["a degree", field("degree", ">", "3"), "degree:>3"],
  ["a version", field("version", ">=", "1700000000"), "version:>=1700000000"],
  ["a group", field("group", "", "Ana"), "group:Ana"],
  ["a group with a space", field("group", "", "Ana B"), 'group:"Ana B"'],
  ["a degree up to", field("degree", "<=", "2"), "degree:<=2"],
  ["a field with an empty value", field("id", "", ""), 'id:""'],
  ["an and", { kind: "and", of: [text("a"), text("b")] }, "a AND b"],
  ["an or", { kind: "or", of: [text("a"), text("b")] }, "a OR b"],
  ["an or inside an and", { kind: "and", of: [text("a"), { kind: "or", of: [text("b"), text("c")] }] }, "a AND (b OR c)"],
  ["an or before an and", { kind: "and", of: [{ kind: "or", of: [text("a"), text("b")] }, text("c")] }, "(a OR b) AND c" ],
  ["an and inside an or", { kind: "or", of: [{ kind: "and", of: [text("a"), text("b")] }, text("c")] }, "(a AND b) OR c"],
  ["a not", { kind: "not", of: text("a") }, "NOT a"],
  ["a not of a group", { kind: "not", of: { kind: "and", of: [text("a"), text("b")] } }, "NOT (a AND b)" ],
  ["a not of a not", { kind: "not", of: { kind: "not", of: text("a") } }, "NOT (NOT a)" ],
  ["a not is never bracketed inside an and", { kind: "and", of: [text("a"), { kind: "not", of: text("b") }] }, "a AND NOT b"],
  ["a not inside an or", { kind: "or", of: [text("a"), { kind: "not", of: text("b") }] }, "a OR NOT b"],
  ["a not of a group inside an and", {
    kind: "and",
    of: [{ kind: "not", of: { kind: "or", of: [text("a"), text("b")] } }, text("c")],
  }, "NOT (a OR b) AND c"],
  ["three terms", { kind: "and", of: [text("a"), text("b"), text("c")] }, "a AND b AND c"],
];

for (const [name, query, printed] of PRINTED) {
  test(`${name} prints as written, and reads back as the same tree`, () => {
    assert.equal(printQuery(query), printed);
    assert.deepEqual(parseQuery(printed), query);
  });
}

// A tree the parser never builds, because `(a AND b) AND c` is the join of three: the printer
// assumes that and writes it tight. Only the printed line is pinned, since the tree it names is
// the join of three, not a join holding a join.
test("a join of the same kind inside itself prints tight, as a parsed tree never holds one", () => {
  const nested: Query = { kind: "and", of: [text("a"), { kind: "and", of: [text("b"), text("c")] }] };
  assert.equal(printQuery(nested), "a AND b AND c");
  assert.deepEqual(parseQuery("a AND b AND c"), { kind: "and", of: [text("a"), text("b"), text("c")] });
});
