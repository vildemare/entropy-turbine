# Combat

Part of the [game design](game_design.md). Who fires, and when a group arrives, is in [Enemies](enemy.md).

There are no melee enemy types in the planned game. Mere contact with an enemy does not damage the player. Once a player is in range and the line of fire is clear, each enemy stops to fire a short burst of three shots. It then moves sideways while reloading before stopping for another burst. Independent opening delays and reload lengths keep groups from firing in unison. When the line of fire is blocked, an enemy tries to change position instead of shooting into an ally or cover.

An enemy holds fire when a friendly unit or cover blocks the line to its target. A shot already in flight disappears if it hits another enemy, but causes no friendly damage. Scout shots deal 150 damage, trooper shots 200, and heavy shots 300 against the player's 1000 starting health. Future minor hazards could deal 100 and still trigger the damage sound without immediately consuming a whole old-style hit. Player shots damage enemies according to their health. Both sides' shots disappear when they hit cover or leave the playfield. Enemies reaching the rear area lose their outstanding shots, stop firing, and retreat toward the side rocks before leaving.

Straight shots stop on barricades, rubble, and the side boulders. The [Steamist](players/steamist.md)'s canister is the exception to plan for: the flight passes over cover and does not strike actors, and the burst at the landing cross is what damages. The [Gunslinger](players/gunslinger.md)'s plasma and enemy gunfire stay direct. The [Mage](players/mage.md)'s fire does not block shots either; it threatens bodies that stand in it.

Keep collision simple: swept projectiles against round actor footprints and box-shaped cover, plus lightweight separation between enemies. Realistic rigid-body physics is unnecessary for this game.

A round carries its material, its delivery, and its damage. The body it meets applies that through its own abilities, in order, so hit points, a flat reduction, or an energy shield each handle their part. An enemy shot that meets another enemy stops there. Cover and the side boulders stop the round and play the impact. When hit points reach zero, the body reports that it has died. A defeated machine then bursts where it stood: a short red flash, a ground ring, and a few brass shards, gone in under half a second. The point orb rises from that spot at 5 m/s, and gains another 5 m/s every 200 ms, up to 100 m/s. Once it is 75 metres clear of the ground, its value is added once to the shared bank.

A flat reduction and a live shield each announce the hit they take, and that announcement plays its own short sound. A body with its shield up wears a faint blue shimmer. The shimmer hides while the shield is down and returns with the pool.

A grenade blast is its own hit. Damage is full at the center of the circle and nothing at the rim, and every body in that circle handles the hit through its abilities. A gunshot between allies still stops short of hit points. The blast does not.
