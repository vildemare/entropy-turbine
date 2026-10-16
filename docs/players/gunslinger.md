# Gunslinger

Part of the [player cast](../player.md). This is the playable character. The animated model is its stand-in.

The Gunslinger fires from alternating sides. The current test version fires two pale blue-grey triangular plasma projectiles with short, faint trails per chamber, spreading to less than one metre apart at 50 metres. Slug speed is 45.3 metres per second and the flight lasts 1.125 seconds, so range remains about 51 metres. The mesh scale is 0.667 and the hit radius is 0.113 metres. Each projectile deals 200 damage, and a pair that both connects deals 400. A projectile can bounce twice when it arrives within 30 degrees of cover or rock; steeper impacts shatter it. Enemy health, armor, and shields use this same integer scale as the Gunslinger's 1000 starting health. The starting magazine holds six rounds. After any shot, a 1.050-second quiet interval starts. Another shot restarts that interval. When it expires, the quickloader seats each missing chamber 60 milliseconds apart. A partly loaded magazine can still fire during the quiet interval; an empty one waits for the loader. Once loading starts, firing is locked until both drums are full, and running returns to normal speed. The ammo drums turn as chambers load, flash when both are full, and an empty trigger clicks only once until released. Reaching a checkpoint and continuing refills the magazine. Firing and moving together still costs movement speed. Ammunition can eventually be switched between:

| Ammunition | Intended behavior | Upgrade direction |
| --- | --- | --- |
| Heat blob | Reliable direct damage to one target | Direct damage and heat efficiency |
| Explosive | Less direct damage, small area blast | Blast damage and radius |
| Piercing | Faster, lower-damage slug with a chance to pass through a target | Velocity, penetration chance, and retained damage |

The first Gunslinger upgrades are maximum health (+200 and heal 200, capped at 5000), magazine capacity (+1, capped at 20), and quickloader wait (-75 milliseconds, down to 150 milliseconds). The price ladder still reaches 50 and stays there, so these ceilings take many phases to buy out. A clear second phase does not finish them. These are starting values for playtests. Ammunition switching and ammo-specific upgrades remain future work. The current model has one visible revolver, so the alternating muzzle effect and relaxed running pose during reload are gameplay stand-ins for later two-gun and holster art.

Purchased health, capacity, and quickloader levels form the player build. Derive maximum health, chamber capacity, and reload wait from that build whenever it changes and at phase start. Keep current health, loaded chambers, and the running reload timer as combat state. Continuing from a checkpoint restores health and fills the derived chamber capacity.

The lower-right HUD shows one chamber ring per gun, above the controls text. A loaded chamber is white; an empty chamber is black with a white border. Each shot turns that gun's next loaded chamber to the top. Capacity upgrades alternate between the two guns, adding a chamber and widening the affected ring so the icons remain legible.

The blueprint is `character::gunslinger` in `src/character.rs`. Reload waits, the shot interval, and the slug lifetime are `Duration`s. Health and damage stay integers. Distances, speeds, and the slug radius are the floats the world already moves with. The shoot-while-moving factor is the plain decimal 0.667, and the slug mesh scale is 0.667. Pair spread, muzzle offsets, angles, and frame deltas stay floating-point. Seconds are formatted only for the shop display. The chamber ring animation stays floating-point. Purchased levels recompute maximum health, capacity, and reload from that blueprint.
