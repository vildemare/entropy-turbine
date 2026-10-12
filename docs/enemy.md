# Enemies

Part of the [game design](game_design.md). How a phase is counted and paid out is in [Phases](session.md).

Gunners arrive only from the front. Their encounters are tied to cover sites along the forward route, rather than
random enemies appearing around the player. Zap-bots also enter from the front, then drive through the field and
leave off the bottom of the view.

At the start of each phase, prepare a ten-wave queue with cover positions, formations, squad sizes, heavy counts,
reinforcement flags, field cover positions, and the checkpoint position. Consume each wave when it comes into view.
Enemy movement and whether a planned reinforcement squad is actually called remain responsive to play.

1. A cover site is built ahead of the view.
2. When that barricade reaches the top of the screen, a group enters from ahead, either in a row or single file, and reaches cover quickly.
3. Members take positions behind the cover and linger there ducked down.
4. They move around the cover at 1.5 times their usual speed and advance, firing independently. A defender that gains a
   clear line of sight during entry, sheltering, or flanking interrupts that route and attacks immediately.
5. Occasionally, the opening fire calls a second squad from farther ahead. Reinforcements arrive in a row and attack
   without seeking cover.

Groups begin organized but do not stay locked in formation. Some attackers advance mainly along their lane; others head
more directly toward a player. Enemies must keep enough space to avoid walking on top of one another. A walker that is
trying to move and has not advanced for a tenth of a second backs up about a metre, then resumes its route. A clear
shot still stops it during that backup, the same as on any other step.

Start with a few readable variants:

- **Scout:** small, fast, falls to one player hit.
- **Trooper:** standard ranged attacker, takes a couple of hits.
- **Heavy:** larger, slower, and needs several hits.

The current variants use assembled low-poly gunners with coats, helmets, visors, brass tanks, and firearms. Their
proportions and colors distinguish the three roles; these temporary visuals can later be replaced with original
steampunk models.

Enemy shots travel at 13 metres per second. That is slow enough to step aside from while the first groups are still easy
to clear. Damage stays 150, 200, and 300. Later phases can raise this speed when the field is meant to become a
struggle.

## Zap-bots

In this build. These are the slow electric cows: low tracked hulls with a brass battery box and two copper prongs.
They are not Entropy Turbines, and they are not a small version of the boss.

They spawn every several seconds from ahead of the view, a few alive at once, while the gunner waves keep their own
schedule. Each one picks a lane and drives down the road until it leaves off the bottom of the screen. Barricades and
rubble are obstacles, and those obstacles are already built before the cow reaches them. A cow looks about fourteen
metres ahead and starts turning toward the clearer side before it reaches the cover, so the path bends around the
obstacle instead of kissing it. When a cow is trying to drive and has not advanced for a tenth of a second, it backs
up and peels toward the other side, then takes up its lane again. That backup is still driving: the cow does not
stop, and a charge that is due keeps rising and fires.

Each cow wears a plate that takes 1 off a hit. The Gunslinger's round is 2, so 1 gets through, and the plate rings when it does. Health is 2, so one paired hit still drops the cow. They are extra bodies, and killing them is meant to feel easy.

They have no gun. The prongs stay dark until a player steps inside the jolt's reach, about eight metres. That step
starts the charge: one rising whine of about a second and a half, and the prongs brighten with it. At the top of the
rise the cow throws a bright jolt toward the nearest player. The rise finishes and the jolt still leaves if the player
has moved out of reach, and then it travels along the cow's facing. The jolt travels about eight metres, so it can be
stepped off of, and it stops on cover. It stings less than a gunner's shot. The prongs then go dark for a short rest,
and the next whine waits until a player is inside that reach.

## Grenade walkers

In this build. A grenade walker is a low mech with wide feet set out to the sides and a four-tube launcher
on the hull. It arrives with a wave.

The last wave of the first phase always includes one. After that, a later wave includes one about one
time in eight.

It walks a diagonal down the road and stays off cover. When the next step would meet a barricade, rubble,
or the edge of the road, it turns ninety degrees away from that obstacle and continues. Hit points are 4.
In front of those sits a shield of 4 that, once emptied, stays down for 4 seconds and then refills.
While the shield is up the walker wears the faint blue shimmer. A paired plasma hit empties the shield.
Another paired hit during those 4 quiet seconds drops the body.

When a player comes within 16 metres it stops, turns toward that player, and aims for 2 seconds. It then
throws four grenades in one volley. The aim point is the center of the pattern. The four landings are
spread around that center, kept apart, and some of them pass the aim point. Each landing shows a rotating
red cross on the ground with a circle around it. The circle's rim is 2 metres from the cross, and that is
the whole of the blast. Damage is 80 at the center and falls off to nothing at the rim. Every body
standing in a circle takes its share, including other machines and the walker when it is inside one.
When several marks land in the same moment, the player's grace answers the strongest and the lighter
ones find that grace already up. The grenades are in the air for about a second. Reload is one second for each grenade, four seconds for the
volley, and the walker walks during that reload. A clear player starts another aim once the reload is
done.

A defeated walker drops 3 points.

## Entropy Turbines

Not in this build. These are the bosses. One stands at the end of a phase. Each phase's boss is its own machine, and
each one is more dangerous than the boss before it, through to the end of the run.

The first boss hovers a little above the ground on turbine thrust, and that lift is the whole of its movement. It puffs
slow projectiles that do minor damage. While nobody is close enough for a direct zap, it spends the building charge as
a zap in a random direction. A player who steps in is zapped directly. The charge runs down, and from time to time the
machine has to land and spin the turbine back up. On the ground it cannot zap. That pause is short. The hull takes
ordinary fire slowly. Zap ports take damage faster while they are open. They open during a zap, and they open when a
player comes up to a hatch during the spin-up. The fight is learning that rhythm: let it spend the charge, step in
while it is down, and step back before it lifts.

Later bosses differ from that fight. The locked rule is the climb in danger, not one pattern repeated at a larger size.

A spawned machine copies its body, movement, weapon, and abilities from `src/character.rs`. Scout, trooper, and heavy live in `src/enemy/gunner.rs`, beside the zap-bot and the grenade walker. A gunner reports what it can see, and the rifle spends the burst.
A zap-bot carries a jolt and a plate. A grenade walker carries a launcher and a shield. Health, damage, and rewards
stay integers. Distances and speeds use the same metres the world already moves in. Waits are `Duration`s. Cover entry and exit stay plain factors on the gunner's pace. The grenade walker's step and the cow's lane step live with the other locomotion in `src/enemy/behavior.rs`. The launcher owns the aim and the throw. The jolt owns the charge and the bolt. A heavier weapon that also changes
the pace is still to be built. The three gunners keep the behavior they have.

Enemies do not yet react to ground hazards. A [Mage](players/mage.md) patch or a [Steamist](players/steamist.md) burst
can damage them where they stand. Pathing around a patch waits until that hazard is readable on its own.
