# igtap-autosplitter

A LiveSplit auto splitter for [IGTAP: an Incremental Game That's Also a Platformer](https://www.speedrun.com/IGTAP),
running on LiveSplit's sandboxed Auto Splitting Runtime (WebAssembly). Real time only, as speedrun.com times the game.

## Use

In LiveSplit: Edit Layout → + → Control → Auto Splitting Runtime, then pick `igtap_autosplitter.wasm` from the
[latest release](../../releases/latest). The splitter's settings are in that component's settings.

**Run** picks what starts the timer:

| Run | Starts | Ends |
|---|---|---|
| Full game (TouchGrass%, FalseEnding%, VmanCrash%, TrueEnding%) | first movement (stick or jump) in a new game | the run's ending split |
| Course IL (Levels 1–15) | the course's start gate (a restart starts again) | its end gate |
| VmanCrash% IL | buying a Start a VMAN speedrun box | the Vman ending |

Each ticked event splits once per run, when it happens; make your splits match the events your route reaches, in order:
wall jump, dash, double jump, block swap, omnidash, breaker trip, landing in Area 2 (TouchGrass%'s end), each course's
first clear, false ending ("You Win" appears), true ending (the IGTAP logo appears), Vman ending (the credits trigger).
A new game resets a full-game run.

## Build

```sh
rustup target add wasm32-unknown-unknown
cargo b --release                              # target/wasm32-unknown-unknown/release/igtap_autosplitter.wasm
cargo test --target x86_64-pc-windows-msvc     # the split logic, on the host (use your host's triple)
```

It reads the game (Unity, Mono) through [`asr`](https://github.com/LiveSplit/asr): the player's `Movement`, the
`Courses`' `courseScript`s, `globalStats`, `ZoneLoader`, the play `timer`, the credits scripts and
`VmanScript.isCurrentlyVman`, found by their paths in the `Overworld` scene.
