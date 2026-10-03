#!/usr/bin/env python3
"""Emit ``fixtures/scigraphs/lesmis.json`` from SciGraphs' own numbers.

Run in the oracle image, with the SciGraphs tree mounted read-only:

    docker run --rm -v "$PWD:/w" \
      -v "$PWD/SciGraphs:/sg:ro" -w /w \
      ge-python-oracle python3 harness/emit-scigraphs-lesmis.py /sg \
      --out fixtures/scigraphs/lesmis.json

The positional argument is the SciGraphs checkout root; ``/sg`` is the default
(``$SCIGRAPHS_ROOT``). ``--out`` is the document's path and defaults to
``fixtures/scigraphs/lesmis.json``, resolved against the working directory.

The document is written to a temp file in the target's own directory and then
renamed onto the target, so nothing is ever opened on the target path and a
crash — a SciGraphs import error halfway through the layout, a full disk —
leaves the committed fixture byte-for-byte as it was. That is why no shell
redirect is used to capture stdout: the redirect would truncate the committed
file before this process wrote a byte, and stdout is block-buffered into a
pipe, so the truncation would survive the failure that caused it.

``os.replace`` is only atomic within one filesystem, which is why the temp file
is a sibling of the target rather than in ``$TMPDIR``. SciGraphs' own
import-time warnings and the fig6 overlap comparison go to stderr; stdout stays
empty, so two runs of this command produce byte-identical files.
"""

import argparse
import os
import sys
import tempfile

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import scigraphs_lesmis_fixture as fix
import scigraphs_lesmis_graph as gr

DEFAULT_OUT = "fixtures/scigraphs/lesmis.json"
TEMP_PREFIX = ".lesmis-"
TEMP_SUFFIX = ".json.tmp"


def write_atomic(path, text):
    """Put *text* at *path* by rename; the target is never opened for writing.

    The temp file is created in the target's directory so the rename is a
    same-filesystem one, flushed and fsynced before the rename so a crash
    cannot leave a rename over an unwritten inode, and unlinked on the way out
    of every path that did not rename it.
    """
    target = os.path.abspath(path)
    directory = os.path.dirname(target) or "."
    os.makedirs(directory, exist_ok=True)
    handle, temp = tempfile.mkstemp(dir=directory, prefix=TEMP_PREFIX,
                                    suffix=TEMP_SUFFIX)
    try:
        with os.fdopen(handle, "w", encoding="utf-8", newline="\n") as stream:
            stream.write(text)
            stream.flush()
            os.fsync(stream.fileno())
        os.replace(temp, target)
    finally:
        if os.path.exists(temp):
            os.unlink(temp)
    return target


def parse_args(argv):
    """``[SCIGRAPHS_ROOT] [--out PATH]``; argparse exits 2 on a third positional."""
    parser = argparse.ArgumentParser(
        prog="emit-scigraphs-lesmis.py",
        description="Emit the SciGraphs Les Miserables fixture.")
    parser.add_argument("root", nargs="?", default=gr.SCIGRAPHS_ROOT,
                        metavar="SCIGRAPHS_ROOT", help="SciGraphs checkout root")
    parser.add_argument("--out", default=DEFAULT_OUT, metavar="PATH",
                        help="document path (default: %s)" % DEFAULT_OUT)
    return parser.parse_args(argv[1:])


def main(argv):
    args = parse_args(argv)
    gr.scigraphs_setup(args.root)
    scene = fix.make_scene(gr.build_graph())
    written = write_atomic(args.out, fix.dumps(fix.build_document(scene)))
    sys.stderr.write(fix.overlap_report(scene))
    sys.stderr.write("wrote %s\n" % written)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))