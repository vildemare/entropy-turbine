# entropy-synth

A small synthesizer crate in the Entropy Turbine workspace. Sound definitions live here as Rust layers. The game keeps the rendered WAV files and a RON ledger.

From the repo root:

```sh
cargo run -p entropy-synth -- play player_shot
cargo run -p entropy-synth -- play player_shot 4
cargo run -p entropy-synth -- play example
cargo run -p entropy-synth -- list
cargo run -p entropy-synth -- export
```

`play` renders in memory and sends it to the speakers. It does not write a file. `example` is the long plasma shot from the builder notes. The exported `player_shot` variants are that same shape, pressed shorter so shots can overlap in a fight.

`export` writes into this repo at `assets/audio/`. Pass `--game /path/to/entropy-turbine` or set `ENTROPY_GAME` for another checkout. A synthesized WAV is rendered again only when its definition hash in `sounds.ron` differs, the file is missing, or you pass `--force`. Entries marked `origin: recorded` are left alone, so a recorded WAV can be added later:

```ron
(
    id: "horn_call",
    file: "horn_call.wav",
    origin: recorded,
    definition: "",
    duration_secs: 1.2,
),
```

A frequency sweep moves exponentially from the starting pitch to `sweep_to`. A layer with no duration plays until the sound ends. A layer with no envelope stays at full level for its window.

While the game is running under `cargo run`, Bevy's file watcher reloads a WAV after export replaces it. The next time that sound plays, it is the new file. Restart the game only after a code change, such as turning the watcher on.
