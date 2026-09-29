"""The query grammar, evaluated here so a row is an oracle."""

# ------------------------------------------------------------- the grammar, evaluated here

FIELDS = ("id", "tag", "kind", "db", "path", "degree")
OPERATORS = ("<=", ">=", "!=", "<", ">", "=")
KEYWORDS = ("AND", "OR", "NOT")


class Bad(Exception):
    """The probe read the grammar and will not guess at the rest of it."""


def scan(text):
    """The words and the punctuation, with a quoted tail kept on the word it belongs to."""
    out, at = [], 0
    while at < len(text):
        char = text[at]
        if char.isspace():
            at += 1
        elif char in "()":
            out.append((char, None, False))
            at += 1
        elif char in "\"'":
            at = quoted(text, at, out)
        else:
            at = bare(text, at, out)
    return out


def quoted(text, at, out):
    close = text.find(text[at], at + 1)
    if close < 0:
        raise Bad(f"a {text[at]} quote is never closed")
    out.append(("word", text[at + 1:close], True))
    return close + 1


def bare(text, at, out):
    start = at
    while at < len(text) and not text[at].isspace() and text[at] not in "()":
        if text[at] in "\"'":
            at = quoted(text, at, [])
        else:
            at += 1
    word = text[start:at]
    if word in KEYWORDS:
        out.append((word, None, False))
    else:
        out.append(("word", word, False))
    return at


def atom_of(word):
    """One term: free text, or `field:` with an optional operator and a value."""
    head, colon, value = word.partition(":")
    if colon == "":
        return ("text", word.lower())
    field = head.strip().lower()
    if field not in FIELDS:
        raise Bad(f"`{head}` is not a field")
    op = ""
    for candidate in OPERATORS:
        if value.startswith(candidate):
            op, value = candidate, value[len(candidate):]
            break
    if field == "degree":
        if op == "":
            raise Bad("`degree:` needs an operator: one of < <= > >= =")
        if not value.isdigit():
            raise Bad(f"`degree:` needs a whole number, not `{value}`")
        value = int(value)
    if field == "tag":
        value = value.lstrip("#")
    return ("field", field, op, value)


def parse(text):
    tokens = scan(text)
    if not tokens:
        return ("all",)
    node, at = expression(tokens, 0)
    if at != len(tokens):
        raise Bad("a word follows the end of the query")
    return node


def expression(tokens, at):
    """`or`: the loosest binding, so `a OR b AND c` reads as `a OR (b AND c)`."""
    node, at = conjunction(tokens, at)
    while at < len(tokens) and tokens[at][0] == "OR":
        right, at = conjunction(tokens, at + 1)
        node = ("or", node, right)
    return node, at


def conjunction(tokens, at):
    node, at = negation(tokens, at)
    while at < len(tokens) and tokens[at][0] == "AND":
        right, at = negation(tokens, at + 1)
        node = ("and", node, right)
    return node, at


def negation(tokens, at):
    if at < len(tokens) and tokens[at][0] == "NOT":
        node, at = negation(tokens, at + 1)
        return ("not", node), at
    return primary(tokens, at)


def primary(tokens, at):
    if at >= len(tokens):
        raise Bad("the query ends where a term was expected")
    kind, text, _ = tokens[at]
    if kind == "(":
        node, at = expression(tokens, at + 1)
        if at >= len(tokens) or tokens[at][0] != ")":
            raise Bad("a `(` is never closed")
        return node, at + 1
    if kind in (")", "AND", "OR", "NOT"):
        raise Bad(f"`{kind}` has nothing to join")
    return atom_of(text), at + 1


def holds(node, record, broken):
    """One node against one parsed term. `broken` is the negative control and nothing else."""
    kind = node[0]
    if kind == "all":
        return True
    if kind == "and":
        return holds(node[1], record, broken) and holds(node[2], record, broken)
    if kind == "or":
        return holds(node[1], record, broken) or holds(node[2], record, broken)
    if kind == "not":
        inner = holds(node[1], record, broken)
        # Under the break the negation is dropped: `NOT x` is read as `x`.
        return inner if broken else not inner
    if kind == "text":
        return node[1] in record["label"].lower()
    return field_holds(node, record)


def field_holds(node, record):
    _, field, op, value = node
    negated = op == "!="
    if field == "degree":
        return compares(record["degree"], op, value)
    if field == "path":
        # Ponytail: `path:` is read as this path or a directory under it. A studio reading it
        # as a plain prefix disagrees here only on a value left without its slash, and every
        # query in this file carries whole paths, so the row cannot tell the two apart.
        hit = record["path"] == value or str(record["path"]).startswith(value + "/")
    elif field == "tag":
        tags = [str(tag).lstrip("#").lower() for tag in record["tags"]]
        hit = value.lower() in tags
    else:
        hit = str(record[field]) == value
    return (not hit) if negated else hit


def compares(have, op, wanted):
    if op == ">":
        return have > wanted
    if op == "<":
        return have < wanted
    if op == ">=":
        return have >= wanted
    if op == "<=":
        return have <= wanted
    return have == wanted


def visible_set(query, records, broken):
    """The nodes this probe says the query keeps: its own parse, its own evaluation."""
    tree = parse(query)
    return {record["index"] for record in records if holds(tree, record, broken)}
