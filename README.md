# igtap-autosplitter

LiveSplit auto splitter for [IGTAP](https://www.speedrun.com/IGTAP). Real time only.

## Use

Open a splits file from [`splits/`](splits), then right-click LiveSplit → **Edit Splits…** → **Activate**. LiveSplit
downloads the splitter and keeps it updated; **Settings** next to Activate picks the run and which events split.

No Activate button, or testing a local build? Edit Layout → + → Control → Auto Splitting Runtime, and point its Script
Path at `igtap_autosplitter.wasm` from the [latest release](../../releases/latest). `IGTAP.lsl` is a layout with
that component in it: open it (right-click → Open Layout), then Edit Layout → Layout Settings → Auto Splitting Runtime
and set Script Path. Don't also Activate from the splits, or every event splits twice.

Streaming from OBS? [obs-livesplit-one](https://github.com/LiveSplit/obs-livesplit-one) runs LiveSplit One as an OBS
source and can load the same `.wasm` as its local auto splitter.

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

`FalseEnding.lss`, `TrueEnding.lss` and `TouchGrass.lss` follow the TAS routes, course clears included.
`VmanCrash.lss` lists the events in the order above; reorder or remove splits to match your route. It leaves out course
clears, so untick **course clears** or add a split for each clear.

## Build

```sh
rustup target add wasm32-unknown-unknown
cargo b --release
cargo test --release --target x86_64-pc-windows-msvc
```
