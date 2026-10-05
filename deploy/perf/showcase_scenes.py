"""The showcase's scenes, in order: (name, steps). Steps are read by showcase.py `step`.

Each scene opens its own graph, so `showcase record <scene>` replays one scene alone.
"""

VAULT = {"nodes": 1500, "degree": 2, "seed": 7, "shape": "vault"}


def synthetic(**override):
    return ("run", "source.synthetic", {**VAULT, **override})


def layout(name, hold=2.2):
    return [("run", "layout.run", {"id": f"layout.{name}"}), ("hold", hold)]


INTRO = [
    ("chrome", False),
    ("run", "appearance.theme", {"name": "scigraphs"}),
    ("run", "appearance.labels", {"mode": "none"}),
    synthetic(),
    ("run", "layout.run", {"id": "layout.forceatlas2.barnes_hut"}),
    ("run", "appearance.background", {"mode": "aurora"}),
    ("run", "appearance.glow", {"on": True}),
    ("run", "appearance.scale", {"factor": 2.0}),
    ("run", "appearance.glowstrength", {"value": 2.5}),
    ("chrome", False),
    ("caption", "graph-motor", "one deterministic layout engine, bit-identical native and wasm32"),
    ("run", "appearance.animate", {"ms": 3500}),
    ("hold", 4.0),
    ("still", "01-intro"),
]

FORCES = [
    ("cut",),
    synthetic(nodes=700, seed=3),
    ("run", "layout.run", {"id": "layout.forceatlas2.barnes_hut"}),
    ("run", "appearance.labels", {"mode": "auto"}),
    ("chrome", False),
    ("roll",),
    ("caption", "Live forces", "the simulation runs in a worker; every knob is a studio action"),
    ("run", "forces.animate", {"on": True}),
    ("hold", 2.0),
    ("caption", "Drag a node", "the pinned node pulls its neighbourhood through the live layout"),
    ("drag", 300, 4.0),
    ("hold", 1.0),
    ("caption", "Spread, then compact", "presets over the same nine knobs"),
    ("run", "forces.spread"),
    ("hold", 1.5),
    ("run", "view.fit"),
    ("hold", 1.5),
    ("run", "forces.compact"),
    ("hold", 1.5),
    ("run", "view.fit"),
    ("hold", 1.5),
    ("still", "02-forces"),
    ("run", "forces.reset"),
    ("run", "forces.animate", {"on": False}),
    ("run", "appearance.labels", {"mode": "none"}),
]

EDGES = [
    ("cut",),
    synthetic(),
    ("run", "layout.run", {"id": "layout.forceatlas2.barnes_hut"}),
    ("run", "appearance.glow", {"on": False}),
    ("run", "appearance.background", {"mode": "theme"}),
    ("run", "appearance.edgecolour", {"mode": "flat"}),
    ("run", "appearance.edgestyle", {"style": "straight"}),
    ("chrome", False),
    ("roll",),
    ("caption", "Edges", "straight, curved, gradient-coloured"),
    ("hold", 1.2),
    ("run", "appearance.edgestyle", {"style": "curve"}),
    ("hold", 1.2),
    ("run", "appearance.edgecolour", {"mode": "gradient"}),
    ("hold", 1.5),
    ("caption", "Edge bundling", "force-directed edge bundling (FDEB) in the motor"),
    ("run", "edges.style", {"id": "post.bundle.fdeb"}),
    ("hold", 2.5),
    ("run", "appearance.background", {"mode": "aurora"}),
    ("run", "appearance.glow", {"on": True}),
    ("run", "appearance.scale", {"factor": 2.0}),
    ("run", "appearance.glowstrength", {"value": 2.5}),
    ("hold", 2.5),
    ("still", "03-bundled-glow"),
    ("run", "edges.style", {"id": "off"}),
]

TOUR = [
    ("caption", "40+ layouts", "every switch tweens from the drawing on screen"),
    *layout("circular.ring"),
    *layout("spiral"),
    *layout("grid"),
    *layout("spectral"),
    *layout("packing.circle"),
    ("still", "04-packing"),
    *layout("circular.radial"),
    *layout("twopi"),
    *layout("force.yifan_hu"),
    ("still", "05-yifan-hu"),
]

ANALYSIS = [
    ("caption", "Analysis", "Louvain communities colour the nodes, betweenness sizes them"),
    ("run", "analysis.run", {"id": "analysis.communities.louvain"}),
    ("run", "appearance.colour", {"by": "analysis"}),
    ("hold", 2.0),
    ("run", "analysis.run", {"id": "analysis.centrality.betweenness"}),
    ("run", "appearance.size", {"by": "analysis"}),
    ("hold", 2.5),
    ("still", "06-centrality"),
    ("caption", "Looks", "the SciGraphs presets, colormap and all"),
    ("run", "appearance.theme", {"name": "ink"}),
    ("hold", 1.6),
    ("run", "appearance.theme", {"name": "blueprint"}),
    ("hold", 1.6),
    ("run", "appearance.theme", {"name": "paper"}),
    ("hold", 1.6),
    ("still", "07-paper"),
    ("run", "appearance.theme", {"name": "scigraphs"}),
    ("run", "appearance.colour", {"by": "group"}),
    ("run", "appearance.size", {"by": "weight"}),
]

SPACE = [
    ("caption", "3D", "the same motor, a third coordinate"),
    ("run", "layout.run", {"id": "layout.forceatlas2.3d"}),
    ("run", "appearance.edgecolour", {"mode": "gradient"}),
    ("run", "appearance.background", {"mode": "aurora"}),
    ("run", "view.fit"),
    ("orbit", {"turns": 1, "tilt": 0.5, "ms": 7000}),
    ("still", "08-space"),
    *layout("basic3d.helix", 1.5),
    ("orbit", {"turns": 0.5, "ms": 3500}),
]

MILLION = [
    ("caption", "1 000 000 nodes", "generated, laid out and drawn in the browser"),
    ("cut",),
    ("run", "layout.run", {"id": "layout.spiral"}),
    synthetic(nodes=1000000, degree=1, shape="random"),
    ("chrome", False),
    ("roll",),
    ("hold", 2.0),
    ("still", "09-million"),
    ("glide", {"zoom": 12, "ms": 6000}),
    ("hold", 1.0),
    ("still", "10-million-zoom"),
]

OUTRO = [
    ("cut",),
    synthetic(),
    ("run", "layout.run", {"id": "layout.forceatlas2.barnes_hut"}),
    ("chrome", True),
    ("roll",),
    ("caption", "graph-motor + studio", "github.com/Univers42/graph_render"),
    ("hold", 3.5),
    ("still", "11-studio"),
]

SCENES = [
    ("intro", INTRO),
    ("forces", FORCES),
    ("edges", EDGES),
    ("tour", TOUR),
    ("analysis", ANALYSIS),
    ("space", SPACE),
    ("million", MILLION),
    ("outro", OUTRO),
]
