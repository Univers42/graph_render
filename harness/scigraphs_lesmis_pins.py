"""Pinned reference values for the SciGraphs Les Miserables fixture.

Every constant here was read off SciGraphs' own code path — the algorithm
parameters from its published specification, the numbers from running that code
— and is asserted from two independent directions: as an exact ``repr`` (so the
fixture's bytes are pinned) and, for the camera, against a vectorised numpy
recomputation written separately in the test modules.
"""

import os

SCIGRAPHS_ROOT = os.environ.get("SCIGRAPHS_ROOT", "/sg")

WORLD_0 = (0.8670820583447432, 4.421097718729663, -3.2740541628998114)
WORLD_76 = (1.7305933342022823, -1.617320655082164, 1.5725793944828328)
CENTRE = (0.6954956493025575, 0.9020302160420557, -0.27889786976097675)
FORWARD = (0.48028825945947246, -0.7204323891892087, 0.5003002702702839)
RIGHT = (0.8320502943378436, 0.554700196225229, -0.0)
UP = (-0.2775166580904616, 0.41627498713569244, 0.8658519732422401)
LOCATION = (8.441304824814255, -10.716683547225491, 7.789653354730376)

R_RADIUS = 6.5874350307280265
DISTANCE = 16.127417281090757
NODE_RADIUS = 0.14492357067601658
EDGE_RADIUS = 0.023056022607548094
TOLERANCE = 0.21738535601402487
TAN_H = 0.3
TAN_V = 0.16875

SCREEN_0 = (1293.8530727177836, 727.4349712706497)
SCREEN_76 = (826.8674518250384, 473.684459331743)
DEPTH_0 = 20.078734054983425
DEPTH_76 = 12.888955472649341

LAYOUT_SEED = 981798123
VALJEAN_ID = 10
BC_VALJEAN = 0.5699890527836184
T_NAPOLEON = 0.27631578947368424
ZERO_BETWEENNESS_NODES = 43

NODE_COUNT = 77
EDGE_COUNT = 254
IN_FRAME = 77
UNOCCLUDED = 65
KEPT = 18

LABEL_IDS = [1, 55, 23, 27, 51, 58, 17, 57, 28, 46, 49, 31, 68, 65, 59, 52, 50, 0]
LABEL_NAMES = ["Myriel", "Marius", "Fantine", "Javert", "MlleGillenormand",
               "Enjolras", "Tholomyes", "Mabeuf", "Fauchelevent", "MmeBurgon",
               "Gillenormand", "Simplice", "Gueulemer", "Joly", "Combeferre",
               "MmePontmercy", "Magnon", "Napoleon"]
OVERLAP = ["Fauchelevent", "Javert", "Mabeuf", "Marius", "MmeBurgon",
           "MmePontmercy", "Myriel", "Napoleon"]
ONLY_OURS = ["Combeferre", "Enjolras", "Fantine", "Gillenormand", "Gueulemer",
             "Joly", "Magnon", "MlleGillenormand", "Simplice", "Tholomyes"]
ONLY_FIG6 = ["Anzelma", "Bossuet", "Champtercier", "Cosette", "Geborand", "Gervais",
             "Labarre", "Marguerite", "MlleBaptistine", "Valjean"]


def check_label_ids(ids=None, node_count=None):
    """Refuse any label id outside ``[0, node_count)``.

    The ids index ``projected.visible``, ``projected.occluded`` and ``by_id``
    directly. A positive out-of-range id raises ``IndexError`` at the use site,
    but a negative one is legal Python indexing and silently aliases the tail —
    id -1 reads node 76 and the pin passes for the wrong node. The bound is
    therefore checked where the ids are defined, which is the only place it can
    be checked without touching the test modules that index them.

    *ids* and *node_count* default to the module's own tables; they exist so
    the check can be exercised on a value that is not the committed one.
    """
    values = LABEL_IDS if ids is None else ids
    count = NODE_COUNT if node_count is None else node_count
    bad = [index for index in values if not 0 <= index < count]
    if bad:
        raise ValueError(
            "LABEL_IDS out of range [0, %d): %s"
            % (count, ", ".join(str(index) for index in bad)))
    return True


check_label_ids()


def mid_ranks(values):
    """SciGraphs' ``_average_ranks``, written again so no pin is circular."""
    order = sorted(range(len(values)), key=lambda i: values[i])
    out = [0.0] * len(values)
    i = 0
    while i < len(order):
        j = i
        while j + 1 < len(order) and values[order[j + 1]] == values[order[i]]:
            j += 1
        for k in range(i, j + 1):
            out[order[k]] = (i + j) * 0.5
        i = j + 1
    return out
