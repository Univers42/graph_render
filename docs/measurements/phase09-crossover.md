# Phase 9 crossover — largest N per arm that fits the frame budget

The headline deliverable is this table, not a pass or fail. Rust faster than the TypeScript oracle at no N would be a stop-and-ask; WASM losing at N = 220 is a row, not a verdict. Every arm's tick is `run / 112` for the same reason and the same way: the motor settles a layout in one call and has no per-tick entry point.

| arm | largest N fitting 16.67 ms | ladder (n, tick ms) |
|---|---:|---|
| native (graph-core, this process) | 10000 | [(220, 0.1451145), (1000, 0.27595721428571424), (2000, 0.7051171160714286), (4000, 1.4110955892857144), (10000, 4.598230098214286)] |
| wasm32 | 4000 | [(220, 0.3102328660714286), (1000, 2.216320196428572), (2000, 5.2560422142857135), (4000, 11.865343607142856), (10000, 35.90474693750001)] |
| TypeScript oracle | 2000 | [(220, 0.5096194999999994), (1000, 3.5553320000000213), (2000, 9.560438499999918), (4000, 19.258147000000008), (10000, 58.56410400000004)] |

Machine class: whatever `graph-cli bench` and the two Node harnesses ran on. Not portable: the harness is a sampler of one machine, one seed, one reference degree. A size past the largest measured is an extrapolation, not a number.
