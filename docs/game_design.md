# Entropy Turbine 3D

Create a small playable top-down 3D shooter prototype in Rust using the latest stable Bevy version (0.19).

The goal is a simple Chaos Engine / Commando / Who Dares Wins style game: move around, aim with the mouse, shoot lots of
enemies, survive, and keep the architecture simple enough to later add networked co-op.

Do not overengineer this. Build a clean, working vertical slice first.

## Core gameplay

Create a 3D top-down shooter with:

- WASD movement
- Mouse aiming
- Left mouse button shoots
- Player always faces toward the mouse cursor projected onto the world plane
- Camera follows the player from a high angled top-down perspective
- Enemies spawn continuously around the player
- Enemies move toward the player
- Enemies damage the player on contact or at close range
- Player projectiles damage enemies
- Enemies die after taking enough damage
- Player has health
- Player can die
- Press R to restart after death

The game should immediately be playable after `cargo run`.

## 3D presentation

Use placeholder 3D geometry only.

For example:

- player: capsule, cuboid, or simple low-poly humanoid placeholder
- enemies: differently colored primitive meshes
- bullets: small spheres or capsules
- ground: large flat plane
- simple directional light
- basic shadows if inexpensive
- simple world obstacles such as boxes

Do not spend time creating polished assets.

Keep the code ready for replacing primitive meshes with GLB/GLTF assets later.

## Player controls

Movement:

- W = forward in world space
- S = backward
- A = left
- D = right

Movement must not depend on camera rotation.

Normalize diagonal movement so diagonal movement is not faster.

Aiming:

- Cast the mouse cursor into the 3D world.
- Intersect the cursor ray with the horizontal gameplay plane.
- Rotate the player toward that world-space position.
- Ignore vertical pitch; rotate only around the Y axis.

Shooting:

- Left mouse button fires toward the aim position.
- Add a sensible fire-rate limit.
- Spawn projectiles slightly in front of the player.
- Projectile direction should be based on where the player is aiming.
- Projectiles disappear after a lifetime or after hitting something.

## Camera

Use a fixed angled top-down camera.

Something approximately like:
- Camera should look down from the above in slight angle, around 25-40 degrees, behind the player.
- the camera should be following the same horizontal axis where the player is
- enough perspective to show the both sides of the top-down corridor

The camera should smoothly follow when player moves forward and backward.

Keep camera behavior simple.

## Enemy spawning

Implement one simple enemy type.

Enemies should:

- spawn outside the player's immediate visible area
- move toward the player
- avoid spawning directly on top of the player
- have health
- take projectile damage
- despawn when dead

Start with continuous spawning.

Increase spawn pressure gradually over time, but keep the rules straightforward.

Do not implement complicated wave logic yet.

## Combat

Use components for important gameplay concepts such as:

- Health
- Player
- Enemy
- Projectile
- Damage
- MovementSpeed
- Lifetime

Prefer small focused Bevy systems.

Avoid huge systems that handle unrelated behavior.

Use Bevy events/messages where they make the gameplay flow cleaner, for example:

- damage applied
- entity killed
- player died

Do not introduce an elaborate generic event architecture.

## Collision

Use a simple collision solution appropriate for this prototype.

If Bevy itself does not provide enough collision functionality for this use case, use a lightweight Bevy-compatible
physics/collision crate.

Collision needs only to support:

- projectile → enemy
- enemy → player
- optional player → obstacle

Prefer simple collider shapes.

Do not create unnecessary realistic physics.

## Game state

Have a small explicit game state such as:

- Playing
- PlayerDead

When Playing:

- movement works
- enemies spawn
- combat runs

When PlayerDead:

- gameplay stops or enemies stop affecting the player
- display a simple death message
- allow restart with R

## UI

Add minimal UI showing:

- player health
- number of kills
- elapsed survival time

When dead, show something like:

"YOU DIED — Press R to restart"

No menus are needed yet.

## Code structure

Use a small understandable project structure.

Something roughly like:

src/
main.rs
player.rs
enemy.rs
combat.rs
camera.rs
game.rs

Adjust this structure if there is a clearly better simple alternative.

Do not create dozens of modules.

Use idiomatic modern Rust and modern Bevy APIs.

Avoid unnecessary abstractions, traits, generic frameworks, or dependency injection.

Prefer explicit data and systems.

## Multiplayer preparation

Do NOT implement networking yet.

However, structure the game so that networked co-op can be added later.

In particular:

- Separate player intent/input from player movement where practical.
- Represent movement and aiming as player commands/input state rather than burying keyboard reads inside unrelated
  gameplay systems.
- Give gameplay entities clear components identifying their role.
- Avoid assumptions that there can only ever be one player entity.
- Systems should generally iterate over players rather than depend on a globally unique player singleton.
- Keep authoritative gameplay state separate from rendering where practical.

The eventual multiplayer model will be networked multiplayer, not multiple people sharing one keyboard.

Do not add networking dependencies yet.

## Future game modes

Do not implement these yet, but avoid architecture that would make them difficult later:

- endless spawning with unlimited respawn
- checkpoint resurrection
- shared team lives
- traditional limited lives
- multiple network players

A later GameMode resource or similar should be able to control these rules.

Do not build the game-mode framework now unless a tiny enum/resource is useful.

## Asset readiness

The project will later use externally generated 3D assets.

Prepare for:

- `.glb` / `.gltf` character assets
- animations
- weapons as separate child objects
- enemy model variants
- environment props

Keep rendering/model setup separate enough that replacing placeholder meshes will not require rewriting gameplay logic.

## Performance assumptions

The eventual game may have large numbers of enemies.

Do not prematurely optimize, but avoid obviously expensive per-entity behavior.

For example:

- do not allocate unnecessarily every frame
- do not repeatedly query assets unnecessarily
- use timers/resources/components sensibly
- keep enemy behavior simple and data-oriented

The initial target can be roughly 50–100 simultaneous enemies without doing anything exotic.

## Deliverables

Produce:

1. Complete Rust source code.
2. `Cargo.toml`.
3. Any required setup files.
4. A short README explaining:
    - how to run it
    - controls
    - project structure
    - where future GLB assets should be added
5. Ensure the project compiles.
6. Run `cargo fmt`.
7. Run `cargo check`.
8. Fix compilation errors before finishing.

Do not stop after generating code. Verify it compiles.

## Development priority

Prioritize in this order:

1. Player movement
2. Mouse aiming
3. Shooting
4. Enemy spawning
5. Enemy chasing
6. Damage/death
7. Camera
8. Minimal UI
9. Small code cleanup

If something fancy conflicts with having a working game, choose the working game.

The result should feel like the smallest possible foundation for a fast, dumb, enjoyable top-down shooter rather than
the beginning of a giant engine architecture project.
