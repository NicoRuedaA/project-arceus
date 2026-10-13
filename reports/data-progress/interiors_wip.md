# Interiors WIP Freeze

- **Freeze date:** 2026-10-13.
- **Retained work:** Part 1 remains committed as `5ea696d` (`docs(data): document subarea coordinate frames`).
- **Part 2:** The uncommitted interior runtime implementation was discarded because its focused test execution was not verified. No viewer or audio work was opened.
- **Checks that passed:** `cargo check -p pla --lib --offline` and `cargo check -p pla --tests --offline`.
- **UNKNOWN boundaries:** Interior runtime behavior, viewer/runtime integration, and final shell/model/navmesh availability semantics remain unverified. No binary-match or native-function claim is made.
