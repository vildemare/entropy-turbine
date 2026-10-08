# Entropy Turbine 3D — game design

A top-down, forward-scrolling Rust/Bevy shooter inspired by classic arcade games such as The Chaos Engine. The player pushes through a hostile steampunk landscape, reads the terrain for danger, and fights organized groups of ranged enemies. Keep each playable slice simple and responsive; avoid adding a general-purpose game framework before it is needed.

These notes are split the same way as `src/`. This page is the current build and the work order. The linked pages hold the rules for each part.

- [Setting](setting.md) — the civilization, the forbidden machines, and who is sent to stop them
- [World](world.md) — the dirt route, camera, and cover as a warning
- [Players](player.md) — shared rules, then [Gunslinger](players/gunslinger.md), [Mage](players/mage.md), and [Steamist](players/steamist.md)
- [Enemies](enemy.md) — waves, formations, and the three variants
- [Combat](combat.md) — shots, bursts, and collision
- [Phases](session.md) — ten-wave phases and what a crossing does
- [Menus](ui.md) — pause and the checkpoint shop
- [Audio](audio.md)
- [Code and co-op](code.md)

## Current playable slice

The playable character is the Gunslinger. The Mage and the Steamist are specified and are not in this build.

- WASD moves in world space. Diagonal movement is normalized.
- The mouse aims on the horizontal world plane; holding the left button fires at a limited rate.
- Shooting costs speed: player movement and the run animation play at two thirds of their normal rate while fire is held.
- The animated player model's legs follow movement. Shooting poses and torso twist follow the mouse; extreme aim angles turn the whole body rather than twisting the spine beyond 90 degrees.
- The camera views the full width of the route from a high angle. It scrolls forward only. The player may retreat within the current view but cannot leave through its rear edge. Forward tracking resumes once the player crosses the camera's ground centerline.
- The player starts at 1000 health, can die from enemy gunfire, and can restart with R. The HUD shows phase, wave, kills, points, survival time, two revolver chamber rings, and five thin green health blocks. Each block represents 20% of current maximum health; the filled count is rounded up, and spent blocks turn black.
- Each phase has ten barricade waves. A brass crossing line after the tenth wave opens a checkpoint shop; Continue begins the next, harder phase.
- Defeated enemies leave collectible point orbs. Scouts give 1 point, troopers 2, and heavies 4. An enemy that managed to damage a player adds 1 point to its drop, once per enemy. Nearby orbs drift toward the player.
- Escape pauses gameplay and opens Continue and Exit Game controls. The pause menu uses the same typography planned for later menus.
- Original short sounds mark the player's plasma fire, enemy gunfire, impacts, damage, death, and menu actions. A quiet machinery-and-wind loop fills the background and pauses with gameplay.

## Immediate development priorities

1. Keep movement, aiming, shooting, and restart responsive.
2. Make cover visibly telegraph incoming groups.
3. Make row/file arrival, shelter, flanking, and ranged attacks readable.
4. Make enemy silhouettes and hit counts distinct.
5. Replace placeholder terrain and enemy visuals with original low-poly art in later passes.
6. Playtest checkpoint earnings and upgrade prices. Ammunition choices, the Mage, the Steamist, and original character art wait until this slice stays readable.

The game should run directly with `cargo run`; run `cargo fmt` and `cargo check` after changes and fix compilation errors before finishing.
