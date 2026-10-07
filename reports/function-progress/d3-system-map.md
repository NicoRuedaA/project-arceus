# D3 — structural map of the game-side code of update `main`

Scope: the 117,455 game-side functions of update-v262144 `main` (everything D2 did not identify as a third-party library; an upper bound for game code). Metadata only: no strings, names or pseudocode. Data: [`d3-system-clusters.tsv`](d3-system-clusters.tsv) (one row per cluster) and [`d3-function-cluster.tsv`](d3-function-cluster.tsv) (function → cluster).

## Method

1. Build an undirected graph of the game-side functions: call and tail-jump edges (excluding 312 hub functions called by more than 60 game functions, such as allocators and guard helpers) plus a weak edge between functions adjacent in address (translation units are linked contiguously).
2. Partition it with Louvain community detection (fixed seed; hub threshold 60, adjacency weight 2.0, resolution 1.0).
3. Label a cluster only when at least 2 of its functions reference resource paths of one system (screens, effects, characters, events, save) and at least 60 % of them agree.

## Result

| Metric | Value |
|---|---:|
| Game-side functions clustered | 117,455 |
| Clusters | 393 |
| Clusters with ≥ 1,000 / 100–999 / 20–99 / < 20 functions | 26 / 131 / 76 / 160 |
| Functions in clusters under 20 functions | 1,258 |
| Clusters with label evidence | 12 (ui ×7, save ×2, effect, event_script, havok_integration) |
| Functions in labelled clusters | 18,584 (15.8 %) |
| Functions in clusters with no label evidence | 98,871 (84.2 %) |

## Validation

- **Cohesion of known groups:** the 34 handlers of the D0 pilot class land in a single cluster (34/34); 9 of the 10 documented animation functions share one cluster.
- **Purity against resource-path anchors:** 65 of 88 anchored functions (74 %) sit in a cluster dominated by their own top-level system.
- **Sensitivity:** six other parameter sets gave 393–487 clusters; the pilot class stayed together in 25–34 of 34 and the animation functions in 8–9 of 10.
- **Adjacency is essential:** without the address-adjacency edge the call graph alone splits into 39,240 communities (35,590 singletons), because most calls to game code are indirect.
- **Address locality:** the median large cluster (≥ 100 functions) has 75 % of its members inside its densest 1 MB address window.

## Limits

- Clusters are structural candidates for systems, not systems. A cluster can merge several systems or split one.
- 84.2 % of game-side functions have no label evidence: the game's strings are mostly stripped, there is no class-type information in the binary, and the names of the Lua-bound classes cannot be tied to their functions (identical strings are merged across the binary). No label is inferred from structure alone.
- Library hub exclusion and the hub threshold are parameters; a different threshold moves boundary functions.

## Next

D4 names clusters as analysis produces direct evidence for them; labels are added to `d3-system-clusters.tsv` only with that evidence.

## Reproduction

Python (`networkx` Louvain) over the D2 call-graph and function tables in `work/d3/` (git-ignored); inputs come from the read-only scripts `CallEdges`, `FuncList` and `StringAnchors`.
