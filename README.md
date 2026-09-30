# igtap-autosplitter

LiveSplit auto splitter for [IGTAP](https://www.speedrun.com/IGTAP). Real time only.

## Use

Edit Layout → + → Control → Auto Splitting Runtime, then load `igtap_autosplitter.wasm` from the
[latest release](../../releases/latest).

| Run | Starts | Ends |
|---|---|---|
| Full game | first movement in a new game | the ending split |
| Course IL | the course's start gate | its end gate |
| VmanCrash% IL | buying the Start a VMAN speedrun box | the Vman ending |

Each enabled event splits once per run:

- wall jump, dash, double jump, block swap, omnidash
- breaker trip
- touch grass (the well trigger, TouchGrass% end)
- each course's first clear (Courses 11–15 are 1–5 with Area 1 overgrown)
- false ending, true ending, Vman ending

Starting a new game resets a full-game run.

## Build

```sh
rustup target add wasm32-unknown-unknown
cargo b --release
cargo test --release --target x86_64-pc-windows-msvc
```
