# Entropy Turbine

A forward-scrolling 3D shooter prototype built with Rust and Bevy 0.19. The player uses an included animated glTF character; the rocky terrain and steampunk enemy gunners are assembled from simple 3D meshes.

## Run

Install a current stable Rust toolchain, then run:

```sh
cargo run
```

The first build may take several minutes while Bevy compiles. On Linux, Bevy also needs the usual windowing, graphics, and audio development libraries installed.

## Controls

- **WASD:** move in world space
- **Mouse:** aim on the ground plane
- **Left mouse button:** fire continuously (movement and run animation slow to two thirds speed while held)
- **Escape:** pause or continue; the pause menu also offers Exit Game
- **R:** restart after death

Move along a 26 meter wide dirt route bounded by boulders. The camera only scrolls forward: you can retreat within the current view, but cannot leave through its rear edge. Each phase has ten barricade waves, followed by a brass checkpoint line. Cross it to spend collected points on maximum health, magazine capacity, or a shorter quickloader wait, then continue to a harder phase with full health. Each upgrade type has its own price ladder of 10, 12, 14, 18, 24, 32, 42, then 50 points. The Gunslinger fires six alternating paired plasma rounds. After 1.050 seconds without a shot, the quickloader fills missing chambers 60 milliseconds apart; another shot restarts the wait. Shooting is locked while the loader runs and resumes when the magazine is full. Two chamber rings at the lower right show loaded white and empty black chambers; the next chamber rotates to the top after a shot, and added capacity widens the affected ring. Groups enter in rows or single file, duck behind cover, then flank and attack with guns. They fire three-shot bursts and move evasively while reloading. Some engagements draw a second squad that enters in a row and attacks without cover. Small solid rubble pieces provide extra cover between barricades. Scouts fall to one paired round when both projectiles hit; troopers and heavies take more. Their shots deal 150, 200, or 300 damage against the player's 1000 starting health. Enemies that reach the rear of the view stop firing and retreat toward the side rocks before disappearing. Defeated enemies leave point orbs worth 1, 2, or 4 points by type, plus one if they damaged you. The HUD shows phase, wave, five green health blocks, slugs, kills, points, and survival time. Each health block covers 20% of maximum health and turns black after that range is spent.

Short synthetic effects mark shots, impacts, damage, death, and menu actions. Gunshots randomly choose among three lower-pitched WAV variants per side. A low machinery drone and wind loop sit underneath combat. Gameplay audio pauses with the Escape menu.

## Share a Windows build

The current game loads its models, fonts, and sounds from `assets/` beside the executable. Send a ZIP containing both; your son can extract the folder and double-click `entropy-turbine.exe` without installing Rust.

On a Windows build machine with Rust and the Visual Studio C++ build tools installed:

```sh
cargo build --profile share
python tools/package_windows.py target/share/entropy-turbine.exe
```

On Linux, install the `x86_64-pc-windows-gnu` Rust target and the MinGW-w64 cross compiler first, then build and package:

```sh
rustup target add x86_64-pc-windows-gnu
CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=x86_64-w64-mingw32-gcc cargo build --profile share --target x86_64-pc-windows-gnu
python3 tools/package_windows.py target/x86_64-pc-windows-gnu/share/entropy-turbine.exe
```

The resulting `dist/entropy-turbine-windows.zip` is the file to send. The `share` profile removes symbols and optimizes for size; on this Pop!_OS machine it reduced the ZIP from about 47 MB to about 13 MB. Extract it on Windows before launching the game.

## Code

- `src/input.rs` translates keyboard and mouse state into `PlayerIntent` for the keyboard-controlled player.
- `src/attributes.rs` holds base player, Gunslinger weapon, and enemy variant tuning values.
- `src/player.rs` holds player intent, stats, movement, and firing. Gameplay reads intent without reading devices.
- `src/ammo_hud.rs` draws and updates the Gunslinger's two chamber rings.
- `src/enemy.rs` places cover, spawns formations, runs enemy behavior, spacing, and ranged attacks.
- `src/combat.rs` handles projectiles, health, cover hits, friendly shot interception, and point pickups.
- `src/camera.rs` advances the camera with the player and computes the rear movement limit.
- `src/character.rs` loads the player model, switches between movement and shooting animations, and turns the torso toward the mouse while firing.
- `src/street.rs` draws scrolling dirt and boulder boundaries.
- `src/game.rs` sets up the world, stores shared mesh handles, and manages the HUD, checkpoints, and restart.
- `src/sound.rs` loads and plays the bundled effects and pauses gameplay audio with the menu.

The player model is at `assets/models/toon_soldier.gltf`. It comes from the [Quaternius Toon Shooter Game Kit](https://quaternius.com/packs/toonshootergamekit.html), released under [CC0](https://creativecommons.org/publicdomain/zero/1.0/). Its palette was changed to charcoal, leather, and brass, and only the revolver mesh is shown. The glTF keeps its original animations. Future custom characters can replace this visual scene without changing player movement or collision. Input is already represented separately from movement, which leaves room for network supplied player intent later.

Menu headings use [Cinzel](https://github.com/google/fonts/tree/main/ofl/cinzel), and controls use [Oxanium](https://github.com/google/fonts/tree/main/ofl/oxanium). Both are bundled under the SIL Open Font License; their license texts are in `assets/fonts/`.

The sounds in `assets/audio/` are original synthesized WAV files. Run `python3 tools/generate_sfx.py` to regenerate them without external packages.
