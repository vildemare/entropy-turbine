# Entropy Turbine 3D — game design

A top-down, forward-scrolling Rust/Bevy shooter inspired by classic arcade games such as The Chaos Engine. The player pushes through a hostile steampunk landscape, reads the terrain for danger, and fights organized groups of ranged enemies. Keep each playable slice simple and responsive; avoid adding a general-purpose game framework before it is needed.

## Current playable slice

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

## Player cast and gunslinger progression

Plan for four selectable player characters with distinct weapons and upgrade paths. Only the **Gunslinger** is defined so far; the current animated player is its visual stand-in. Keep the other three roles open until their play styles are clear. Every character can upgrade maximum health. Avoid adding a shared energy pool unless a later ability genuinely needs one; health, ammunition, and points are enough for the first progression pass.

The Gunslinger fires from alternating sides. The current test version fires two pale blue-grey triangular plasma projectiles with short, faint trails per chamber, spreading to less than one metre apart at 50 metres. They travel one-third faster than the first paired-shot version and have two-thirds of its visual size and hit radius; flight time is shorter so range remains about 51 metres. Each projectile deals two-thirds of the original single-shot damage; enemy health uses three internal units per old hit point so damage stays integral. A pair that both connects deals four-thirds of the old shot's damage. The starting magazine holds six rounds. After any shot, a 1.050-second quiet interval starts. Another shot restarts that interval. When it expires, the quickloader seats each missing chamber 60 milliseconds apart. A partly loaded magazine can still fire during the quiet interval; an empty one waits for the loader. Once loading starts, firing is locked until both drums are full, and running returns to normal speed. Reaching a checkpoint and continuing refills the magazine. Firing and moving together still costs movement speed. Ammunition can eventually be switched between:

| Ammunition | Intended behavior | Upgrade direction |
| --- | --- | --- |
| Heat blob | Reliable direct damage to one target | Direct damage and heat efficiency |
| Explosive | Less direct damage, small area blast | Blast damage and radius |
| Piercing | Faster, lower-damage slug with a chance to pass through a target | Velocity, penetration chance, and retained damage |

The first Gunslinger upgrades are maximum health (+200 and heal 200, capped at 3000), magazine capacity (+1, capped at 12), and quickloader wait (-75 milliseconds, down to 600 milliseconds). These are starting values for playtests. Ammunition switching and ammo-specific upgrades remain future work. The current model has one visible revolver, so the alternating muzzle effect and relaxed running pose during reload are gameplay stand-ins for later two-gun and holster art.

Purchased health, capacity, and quickloader levels form the player build. Derive maximum health, chamber capacity, and reload wait from that build whenever it changes and at phase start. Keep current health, loaded chambers, and the running reload timer as combat state. Continuing from a checkpoint restores health and fills the derived chamber capacity.

The lower-right HUD shows one chamber ring per gun, above the controls text. A loaded chamber is white; an empty chamber is black with a white border. Each shot turns that gun's next loaded chamber to the top. Capacity upgrades alternate between the two guns, adding a chamber and widening the affected ring so the icons remain legible.

Reload settings use integer milliseconds, and the live reload and shot cooldowns use Rust `Duration` values from Bevy's clock. Health and damage use unsigned integers. Seconds are formatted only for the shop display. Position, movement, and the chamber ring animation still use floating-point values.

## Checkpoint shop plan

Collectables award points during a level. The checkpoint shop shows phase, kills, banked points, and three functioning upgrades. Each row shows its current value, next value, and cost. Unaffordable and maxed offers are dimmed; clicking them has no effect. Continue starts the next phase. The shop uses the same menu frame, buttons, and font pair as pause.

Each upgrade type has its own price ladder: **10 → 12 → 14 → 18 → 24 → 32 → 42 → 50**, then stays at **50**. Buying health only raises the next health price; capacity and quickloader prices rise when those upgrades are bought. The first phase offers up to 93 base points if every enemy and reinforcement is defeated and every orb collected. Actual earnings should be lower when enemies escape, reinforcements do not arrive, or drops go uncollected. Tune this after playtests.

```text
CHECKPOINT SHOP
PHASE 1 CLEARED            KILLS  ...     POINTS  ...
  MAX HEALTH           1000 → 1200          10 P
  SLUG CAPACITY           6 → 7            10 P
  QUICKLOAD          1.050s → 0.975s        10 P
                 [CONTINUE TO NEXT PHASE]
```

When more characters are added, show one selected character's offers at a time. The roster can share the frame and health row while swapping the weapon and ammunition section. The shop opens only at a checkpoint, so Escape remains a separate pause menu during live play. Later, add heat, explosive, and piercing ammunition rows when those shots are implemented.

Use an engraved-looking heading and a readable technical face for costs and details. The current font pair is Cinzel for headings and Oxanium for controls, with earth, soot, iron, and brass UI colors. Revisit this pairing after seeing it at game resolution.

## World and visual direction

The opening stretch is dirt and rock, bounded by continuous boulders. The route has occasional gentle cubic Bezier bends. Its centerline may shift up to four metres either side of the starting axis, and its sideways slope stays below 15% so aiming remains readable. The camera follows the route sideways without rotating and keeps its forward-only behavior. The dirt strip, visible side boulders, player and enemy movement bounds, and encounter positions all use the same centerline. The side boulders stop shots, and their inner edges meet the player movement boundary. Do not present it as a paved street. Rubble, barricades, and other cover should fit a darker steampunk palette: earth, soot, iron, leather, and aged brass. Primitive geometry is acceptable while game behavior is being established.

Later, allow tighter bends where the route visibly splits into two paths and rejoins. The ordinary path keeps the expected difficulty; the harder path offers more points. The split should be clear early enough to choose deliberately, and both paths must lead back into the same progression. Keep the current gentle bends until the branch layout and camera behavior are designed together.

A cover site is a warning to the player. Place barricades or rubble within view before its defenders arrive, so visible cover predicts an encounter. The central wall and both visible end blocks have matching solid bounds that stop projectiles and actors. Two smaller solid rubble pieces between sites give the player places to break incoming fire. Defenders shelter behind the main wall, then leave by its sides. Enemies that reach the camera's rear area stop firing and retreat toward the side rocks; they disappear at the edge or after a short delay if blocked.

The current player visual is a recolored, animated Toon Shooter Kit character. It is a temporary reference for later original characters. Keep model loading and animation separate from the player's movement, collision, health, and input. Future `.glb`/`.gltf` characters, weapons, and props should replace visuals without changing core combat rules.

## Enemy encounters

Enemies arrive only from the front. Encounters are tied to cover sites along the forward route, rather than random enemies appearing around the player.

At the start of each phase, prepare a ten-wave queue with cover positions, formations, squad sizes, heavy counts, reinforcement flags, field cover positions, and the checkpoint position. Consume each wave when it comes into view. Enemy movement and whether a planned reinforcement squad is actually called remain responsive to play.

1. A cover site appears in view.
2. After a short warning, a group enters from ahead, either in a row or single file, and reaches cover quickly.
3. Members take positions behind the cover and linger there ducked down.
4. They move around the cover at 1.5 times their usual speed and advance, firing independently. A defender that gains a clear line of sight during entry, sheltering, or flanking interrupts that route and attacks immediately.
5. Occasionally, the opening fire calls a second squad from farther ahead. Reinforcements arrive in a row and attack without seeking cover.

Groups begin organized but do not stay locked in formation. Some attackers advance mainly along their lane; others head more directly toward a player. Enemies must keep enough space to avoid walking on top of one another.

Start with a few readable variants:

- **Scout:** small, fast, falls to one player hit.
- **Trooper:** standard ranged attacker, takes a couple of hits.
- **Heavy:** larger, slower, and needs several hits.

The current variants use assembled low-poly gunners with coats, helmets, visors, brass tanks, and firearms. Their proportions and colors distinguish the three roles; these temporary visuals can later be replaced with original steampunk models.

Later, build enemy capabilities from explicit parts, as with the player's upgrade build: a body type, weapon, and any special traits should produce movement, aim, health, and attack stats when the enemy spawns. The combination matters: a small enemy carrying a heavy gun should move and aim more slowly than that body would with a light weapon. This should make new enemy and weapon combinations easy to plan and balance without scattering stat adjustments through movement and shooting code. The current three enemy types are working well, so leave their behavior as it is while other parts of the game take priority.

## Audio direction

Keep effects short and readable in a dense fight: a low electrical thump for player plasma, dry mechanical crack for enemy guns, metal impact against cover, and a heavier warning for player damage. The current WAV set is synthesized locally and can be regenerated with `tools/generate_sfx.py`. Player and enemy gunfire each randomly select from three lower-pitched tonal variants to soften repetition. Keep using WAVs while exploring the sound palette; later, define sound profiles in Rust when weapons and enemy capabilities need sounds that react to their properties. Avoid a constant loud soundtrack; leave room for shot timing and a low atmospheric bed. Revisit levels and sound design after hands-on playtests.

## Ranged combat and cover

There are no melee enemy types in the planned game. Mere contact with an enemy does not damage the player. Once a player is in range and the line of fire is clear, each enemy stops to fire a short burst of three shots. It then moves sideways while reloading before stopping for another burst. Independent opening delays and reload lengths keep groups from firing in unison. When the line of fire is blocked, an enemy tries to change position instead of shooting into an ally or cover.

An enemy holds fire when a friendly unit or cover blocks the line to its target. A shot already in flight disappears if it hits another enemy, but causes no friendly damage. Scout shots deal 150 damage, trooper shots 200, and heavy shots 300 against the player's 1000 starting health. Future minor hazards could deal 100 and still trigger the damage sound without immediately consuming a whole old-style hit. Player shots damage enemies according to their health. Both sides' shots disappear when they hit cover or leave the playfield. Enemies reaching the rear area lose their outstanding shots, stop firing, and retreat toward the side rocks before leaving.

Keep collision simple: swept projectiles against round actor footprints and box-shaped cover, plus lightweight separation between enemies. Realistic rigid-body physics is unnecessary for this game.

## Game flow and future phases

The rocky approach is divided into phases of ten cover-led waves. Barricades are spaced roughly 31–38 metres apart. The first phase starts with three enemies per wave, adds one after every three waves, and introduces a heavy from wave five onward. Waves four, seven, and ten call a three-enemy reinforcement row after the defenders engage. This yields 42 defenders and up to 9 reinforcements, worth 93 base points if every enemy is defeated and every orb collected, plus at most one hit bonus per enemy. The next phase adds one defender to each wave, brings two heavies in its last two waves, and has one extra reinforcement wave. Later phases cap the main group at seven. These values are a starting balance for playtesting.

A crossing line appears beyond wave ten. Crossing it clears remaining attackers and projectiles, freezes combat, and opens the upgrade shop. Continue preserves points, restores health to the upgraded maximum, refills the magazine, advances the phase, and resumes spawning ahead. The exact resurrection, lives, and co-op rules remain undecided; do not build a mode framework yet. Avoid code that requires only one possible player or one permanent enemy type.

## Code and multiplayer preparation

`src/attributes.rs` is the tuning source for the base player, Gunslinger weapon, and enemy variants. Purchased levels derive player statistics from those values; enemy spawns copy their variant profile into runtime health, movement, and attack state. Projectiles carry their own speed and hit radius from the firing weapon, so later weapons can vary these properties without changing shared projectile movement. Keep live timers, rounds, health, and AI state on entities.

Use small focused Bevy systems and components for player intent, movement, health, enemies, projectiles, cover, and encounter state. `PlayerIntent` records desired movement, world-space aim, and whether the fire control is held. The keyboard/mouse adapter writes that intent before gameplay runs; movement, weapons, and animation then apply the game's rules. `KeyboardMouseControlled` marks the input source, `LocalPlayer` marks the camera and HUD subject, and `PlayerSlot` gives a stable in-run target for another input adapter. A future gamepad, network, screen-share, or MCP bridge can feed the same intent for its assigned slot without reading devices inside gameplay systems. An ordinary screen-share keyboard/mouse stream controls the existing local slot; a second simultaneous player needs a separate input channel. Enemy attack timing and group behavior live on the relevant entities. Keep the rendering/model setup separate from combat state.

Networking is not implemented. The eventual multiplayer model is networked co-op, not multiple people on one keyboard. Gameplay movement, aiming, shooting, and enemy targeting already iterate over players. Co-op still needs decisions and implementation for camera framing, per-player HUD and shop selection, checkpoint crossing, death/revival, and player-to-player spacing. The current menus use a single-player shop and the phase can end when any player crosses the checkpoint; do not treat those as finished co-op rules.

## Immediate development priorities

1. Keep movement, aiming, shooting, and restart responsive.
2. Make cover visibly telegraph incoming groups.
3. Make row/file arrival, shelter, flanking, and ranged attacks readable.
4. Make enemy silhouettes and hit counts distinct.
5. Replace placeholder terrain and enemy visuals with original low-poly art in later passes.
6. Playtest checkpoint earnings and upgrade prices, then add ammunition choices and original character art.

The game should run directly with `cargo run`; run `cargo fmt` and `cargo check` after changes and fix compilation errors before finishing.
