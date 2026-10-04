# Test fixtures

**Nothing here is committed.** The fixtures are copyrighted game content:
each collaborator regenerates them from their own copy of the game (see
`REPRODUCE.md` for the exact sources).

Point the tests at your extraction instead of copying files here:

```bash
PLA_FIXTURES=/home/you/work/pla/pk2/romfs_patched cargo test -p pla
```

or copy the files from `REPRODUCE.md`'s fixture table into this directory.
Tests whose fixture is missing print `[skip]` and pass, so a clean clone
stays green.
