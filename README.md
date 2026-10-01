# Entropy Turbine

A small top-down 3D shooter prototype built with Rust and Bevy 0.19. The player uses an included animated glTF character; the street, enemies, and bullets use primitive meshes.

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
- **R:** restart after death

Move forward or backward along a 26 meter wide street. Its marked edges are solid boundaries. Survive as long as possible while enemy spawns gradually accelerate. The HUD shows health, kills, and survival time.

## Code

- `src/player.rs` captures local input as `PlayerIntent` for the `LocalPlayer`, then applies movement, aim, and firing to players.
- `src/enemy.rs` handles timed spawning and pursuit.
- `src/combat.rs` handles projectiles, simple swept hit checks, health, and contact damage.
- `src/camera.rs` follows the player.
- `src/character.rs` loads the player model, switches between movement and shooting animations, and turns the torso toward the mouse while firing.
- `src/street.rs` draws the scrolling street and defines its side boundaries.
- `src/game.rs` sets up the world, stores shared mesh handles, and manages the HUD and restart.

The player model is at `assets/models/toon_soldier.gltf`. It comes from the [Quaternius Toon Shooter Game Kit](https://quaternius.com/packs/toonshootergamekit.html), released under [CC0](https://creativecommons.org/publicdomain/zero/1.0/). Its palette was changed to charcoal, leather, and brass, and only the revolver mesh is shown. The glTF keeps its original animations. Future custom characters can replace this visual scene without changing player movement or collision. Input is already represented separately from movement, which leaves room for network supplied player intent later.
