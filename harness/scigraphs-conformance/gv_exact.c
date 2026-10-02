/* gv_exact -- the node coordinates `gvLayout` leaves behind, printed without rounding.
 *
 * Every Graphviz *text* output rounds. `-Tplain` writes inches through `printdouble`
 * (`lib/common/output.c:66-71`), which is `agxbprint(&buf, "%.5g", v)`; five significant
 * digits at 72 points per inch put every reference coordinate on a 7.2e-4-point grid. A
 * matrix that compares against that grid measures the rendering, not the layout. This
 * program links libgvc directly, reads the DOT, lays it out, and prints `ND_coord(n).x`
 * and `ND_coord(n).y` with `%a`: hex float, an exact round trip, so not one bit is lost
 * between the layout and whatever reads it.
 *
 * `gv_exact.py` in this directory compiles this file once per process and parses stdout.
 *
 * Build (gcc 14, as in the `ge-graphviz-oracle` image):
 *   gcc -O2 -std=c11 -I/opt/graphviz/include gv_exact.c \
 *       -L/opt/graphviz/lib -lgvc -lcgraph -lcdt -Wl,-rpath,/opt/graphviz/lib -o gv_exact
 *
 * Usage: gv_exact <engine> <start-seed> <file.dot>
 *   stdout: one line per node, `<node-name> <x> <y>` in `%a`, DOT declaration order.
 *   No bbox, no translation, no scaling: the numbers are `ND_coord` exactly as
 *   `gvLayout` returned it, in points, y-axis pointing up.
 *
 * `sfdp` prints one benign line on stderr and sets Graphviz's error flag because
 * `remove_overlap` is a stub in an image built without triangulation; the reader strips
 * it rather than this program swallowing it (`harness/gv_plain.py:49-61`).
 */

#include <stdio.h>
#include <graphviz/cgraph.h>
#include <graphviz/gvc.h>
#include <graphviz/types.h>

int main(int argc, char **argv)
{
    GVC_t *gvc;
    graph_t *g;
    node_t *n;
    FILE *fp;
    int rc;

    if (argc != 4) {
	fprintf(stderr, "usage: %s <engine> <start-seed> <file.dot>\n", argv[0]);
	return 2;
    }
    if (!(fp = fopen(argv[3], "r"))) {
	fprintf(stderr, "%s: cannot read %s\n", argv[0], argv[3]);
	return 1;
    }
    /* `gvNextInputGraph` reads `gvc->input_filenames`, and GVC_t is opaque to a client
     * (`lib/gvc/gvc.h:73`), so the DOT comes in through cgraph's own reader, which is
     * the `agconcat` that call ends up in (`lib/common/input.c:530`). */
    if (!(g = agread(fp, NULL))) {
	fprintf(stderr, "%s: %s is not a readable DOT graph\n", argv[0], argv[3]);
	fclose(fp);
	return 1;
    }
    fclose(fp);

    gvc = gvContext();
    /* `-Gstart=N` is `global_def("start=N", AGRAPH)` (`lib/common/input.c:281-286`), which
     * declares `start` with default `N` on the proto graph (`lib/common/input.c:178-192`)
     * and `setSeed` reads it back as `agget(G, "start")` (`lib/neatogen/neatoinit.c:921`).
     * Declaring it here and then setting it is that same statement for one parsed graph. */
    agattr(g, AGRAPH, "start", argv[2]);
    if (agset(g, "start", argv[2]) != 0) {
	fprintf(stderr, "%s: cannot set start=%s\n", argv[0], argv[2]);
	agclose(g);
	gvFreeContext(gvc);
	return 1;
    }
    /* `gvLayout` is `gvlayout_select` followed by `gvLayoutJobs` (`lib/gvc/gvc.c:52-64`). */
    if (gvLayout(gvc, g, argv[1]) != 0) {
	fprintf(stderr, "%s: %s could not lay out %s\n", argv[0], argv[1], argv[3]);
	gvFreeLayout(gvc, g);
	agclose(g);
	gvFreeContext(gvc);
	return 1;
    }

    for (n = agfstnode(g); n; n = agnxtnode(g, n))
	printf("%s %a %a\n", agnameof(n), ND_coord(n).x, ND_coord(n).y);

    rc = fflush(stdout) == 0 ? 0 : 1;
    if (rc)
	fprintf(stderr, "%s: could not write stdout\n", argv[0]);
    gvFreeLayout(gvc, g);
    agclose(g);
    gvFreeContext(gvc);
    return rc;
}