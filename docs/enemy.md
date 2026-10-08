# Enemies

Part of the [game design](game_design.md). How a phase is counted and paid out is in [Phases](session.md).

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

Enemy shots travel at 13 metres per second. That is slow enough to step aside from while the first groups are still easy to clear. Damage stays 150, 200, and 300. Later phases can raise this speed when the field is meant to become a struggle.

## Turbine machines

Not in this build. The lesser machine is bulky and unarmed. It zaps a player who comes close, and it has no gun.

The boss is that machine at a larger size. Turbine thrust holds it a little above the ground, and that lift is the whole of its movement. It puffs slow projectiles that do minor damage. While nobody is close enough for a direct zap, it spends the building charge as a zap in a random direction. A player who steps in is zapped directly. The charge runs down, and from time to time the machine has to land and spin the turbine back up. On the ground it cannot zap. That pause is short. The hull takes ordinary fire slowly. Zap ports take damage faster while they are open. They open during a zap, and they open when a player comes up to a hatch during the spin-up. The fight is learning that rhythm: let it spend the charge, step in while it is down, and step back before it lifts.

Later, build enemy capabilities from explicit parts, as with the player's upgrade build: a body type, weapon, and any special traits should produce movement, aim, health, and attack stats when the enemy spawns. The combination matters: a small enemy carrying a heavy gun should move and aim more slowly than that body would with a light weapon. This should make new enemy and weapon combinations easy to plan and balance without scattering stat adjustments through movement and shooting code. The current three enemy types are working well, so leave their behavior as it is while other parts of the game take priority.

Enemies do not yet react to ground hazards. A [Mage](players/mage.md) patch or a [Steamist](players/steamist.md) burst can damage them where they stand. Pathing around a patch waits until that hazard is readable on its own.
