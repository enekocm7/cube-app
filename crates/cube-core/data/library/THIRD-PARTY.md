# Where the library's algorithms come from

Each set in these files says where its algorithms came from on its `source` line.

| `source` | Meaning |
|---|---|
| `speedcubedb` | Gathered by the project's maintainer from [SpeedCubeDB](https://speedcubedb.com/). The case names, groups and algorithms are SpeedCubeDB's; an algorithm that fails on the puzzle's model was left out |
| `standard` | Well-known short algorithms, written by hand |
| `generated` | Found by `examples/library_generate.rs` or `examples/library_square1.rs` |

Case names of `generated` sets are numbers, not community names, and those
algorithms are searched for short length, not for how well they finger-trick.
