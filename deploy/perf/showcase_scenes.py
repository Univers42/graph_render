"""The showcase's scenes, in order: (name, steps). Steps are read by showcase.py `step`."""

SCENES = [
    ("probe", [
        ("chrome", False),
        ("caption", "graph-motor", "a deterministic graph layout engine, native and wasm32"),
        ("hold", 2.0),
        ("run", "appearance.edgecolour", {"mode": "gradient"}),
        ("run", "appearance.edgestyle", {"style": "curve"}),
        ("hold", 1.0),
        ("run", "appearance.background", {"mode": "aurora"}),
        ("run", "appearance.glow", {"on": True}),
        ("hold", 1.0),
        ("still", "probe-glow"),
        ("glide", {"zoom": 2.0, "ms": 2000}),
        ("hold", 0.5),
        ("run", "layout.run", {"id": "layout.circular.ring"}),
        ("hold", 2.0),
    ]),
]
