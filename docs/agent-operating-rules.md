# Agent operating rules

These rules apply to every agent and session working on this repository (Claude, Codex, Antigravity, humans). They exist because of concrete incidents: two agents overwrote each other in the same worktree, a batch of windowed renders flooded the desktop, and an unbounded process got the whole terminal OOM-killed.

## Goal and scope
- Replicate the **game** (maps, creatures, rules, items, assets) in the user's own Bevy engine. Do **not** replicate the original engine, SDK or system layer, and do not spend effort verifying or binary-matching every function.
- Out of scope unless the user asks again: story/quests/event scripts and game texts/localisation.

## Worktrees and branches
1. **One writer per worktree.** Never start a second agent in a worktree that already has one. Before starting run `git worktree list` and `herdr agent list` to see who works where.
2. Each agent gets its own worktree under `/home/nico/work/decompilacion-worktrees/<name>` on its own branch `feat/<name>`.
3. The main checkout (`/home/nico/work/decompilacion`) is **not** an agent workspace. Merges into `main` are done by the orchestrator only, after tagging `pre-merge-*`, running the checks and regenerating the fix2 profile when Rust changed.
4. Never run `git checkout -- <file>`, `git reset --hard`, `git clean` or `rm` on tracked files you did not write. If you find uncommitted changes you do not recognise, copy them aside and stop.
5. Commit early and often with conventional commits and no AI attribution. A worktree is retired only when its branch is merged and its tree is clean.

## Machine safety
6. Every heavy command (cargo build/test/run, batch converters, scans) runs under `systemd-run --user --scope -q -p MemoryMax=6G -p MemorySwapMax=0 -- <cmd>` with `CARGO_BUILD_JOBS=3`. Stream outputs to disk; never accumulate unbounded data in memory. crates.io is reachable, so cargo's `--offline` flag is optional; adding a Bevy feature or a crate (audio, gamepad, bloom) is allowed when a roadmap task needs it, committing both `Cargo.toml` and `Cargo.lock`.
7. **Never open a visible window** on the user's display. Renders and screenshots are offscreen/headless, in one process, not one process per shot. Even a window created with `visible: false` can flash on the user's Wayland desktop, so run the viewer / `--auto-demo` / `--screenshot` at most ONCE per task (group every view in that single run) and never in a loop. The session also has a Hyprland rule sending windows titled `Pokémon Legends: Arceus — Bevy Port Interactive Viewer` to the hidden `special:agents` workspace; it is not a substitute for this rule.
8. Do not use `pkill -f <pattern>` when the pattern also appears in your own command line.

## Data and reports
9. Game files, pseudocode and heavy outputs stay out of git (`work/`, gitignored). Only small typed indexes (`data/`), code and reports are committed.
10. Unproven things stay unknown and are named as such; a valid output file is not proof that the content is right.
