# Gunslinger

Part of the [player cast](../player.md). This is the playable character. The animated model is its stand-in.

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
