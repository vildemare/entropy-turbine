# Audio

Part of the [game design](game_design.md).

Keep effects short and readable in a dense fight: a low electrical thump for player plasma, a dry mechanical crack for enemy guns, metal on cover, and a heavier warning when the player is hit. Player and enemy gunfire each pick at random from three tonal variants. There is no constant loud soundtrack. A low machinery-and-wind bed sits under the fight and leaves room to hear shot timing.

Sounds are authored in the `entropy-synth` crate in this repo. A definition is Rust layers: oscillators, noise, envelopes, and sweeps. `cargo run -p entropy-synth -- play player_shot` renders that sound and plays it without writing a file, so it can be edited and heard again until it sits right. `cargo run -p entropy-synth -- play example` plays the longer plasma shot the builder was shaped around. The exported player shots are that shape pressed shorter, because the Gunslinger fires every 140 ms.

`cargo run -p entropy-synth -- export` writes WAVs into `assets/audio/` and updates `assets/audio/sounds.ron`. The ledger stores each file, its length, and a hash of the synthesized definition. A WAV is rendered again only when that hash differs, the file is missing, or export is run with `--force`. An entry with `origin: recorded` is left alone, so a recorded WAV can be dropped in later and named in the ledger without the tool replacing it.

The game loads those WAV paths. Bevy's `file_watcher` feature reloads a file when export replaces it. The next time that cue plays, it is the new sound. Restart after a code change. A change to a WAV alone does not need one.

The Gunslinger's plasma thump is the player weapon in the build. The Mage's cast and the Steamist's charge and burst need their own short cues when those weapons exist: a fuel ignition for the patch, a rising pressure tick while the Steamist holds, and a steam crack when the canister lands. Those cues are not recorded yet. They should stay as short as the plasma thump so a dense fight can still be read.
