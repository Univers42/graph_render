#!/usr/bin/env python3
"""Emit ``fixtures/scigraphs/lesmis.json`` from SciGraphs' own numbers.

Run in the oracle image, with the SciGraphs tree mounted read-only:

    docker run --rm -v "$PWD:/w" \\
      -v /sgoinfre/students/dlesieur/graph_render/SciGraphs:/sg:ro -w /w \\
      ge-python-oracle python3 harness/emit-scigraphs-lesmis.py /sg \\
      > fixtures/scigraphs/lesmis.json

The argument is the SciGraphs checkout root; ``/sg`` is the default. JSON goes to
stdout and nothing else does: SciGraphs' own import-time warnings and the fig6
overlap comparison are rerouted to stderr, so two runs of this command produce
byte-identical files.
"""

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import scigraphs_lesmis_fixture as fix
import scigraphs_lesmis_graph as gr


def main(argv):
    if len(argv) > 2:
        sys.stderr.write("usage: emit-scigraphs-lesmis.py [SCIGRAPHS_ROOT]\n")
        return 2
    root = argv[1] if len(argv) == 2 else gr.SCIGRAPHS_ROOT
    gr.scigraphs_setup(root)
    scene = fix.make_scene(gr.build_graph())
    sys.stdout.write(fix.dumps(fix.build_document(scene)))
    sys.stderr.write(fix.overlap_report(scene))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
