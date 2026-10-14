# Menus

Part of the [game design](game_design.md).

The game opens on a title screen with Continue. The survival clock, the waves, and the machinery bed wait there so the window can finish loading. Continue starts the run and captures the pointer. Escape is quiet on that screen.

Escape pauses gameplay during a phase and opens Continue and Exit Game. The pause menu uses the same frame, buttons, and font pair as the checkpoint shop and the title. The shop opens only at a checkpoint, so Escape stays a separate pause menu during live play.

During play the window captures the pointer, and the aim mark is a small brass cross with a dark outline that follows it. That capture is what lets the mark slide with a sideways camera move. The title, the pause menu, the checkpoint shop, and the death screen release the capture and show the system pointer.

## Checkpoint shop

Collectables award points during a level. The checkpoint shop shows phase, kills, banked points, and three functioning upgrades. Each row shows its current value, next value, and cost. Unaffordable and maxed offers are dimmed; clicking them has no effect. Continue starts the next phase.

Each upgrade type has its own price ladder: **10 → 12 → 14 → 18 → 24 → 32 → 42 → 50**, then stays at **50**. The Gunslinger rows stop at 5000 health, 20 chambers, and a 0.150 second quickload. Buying health only raises the next health price; capacity and quickloader prices rise when those upgrades are bought. The first phase offers up to 93 base points if every enemy and reinforcement is defeated. Each bounty rises off the ground and pays the shared bank. Actual earnings should be lower when enemies escape or reinforcements do not arrive. Tune this after playtests.

```text
CHECKPOINT SHOP
PHASE 1 CLEARED            KILLS  ...     POINTS  ...
  MAX HEALTH           1000 → 1200          10 P
  SLUG CAPACITY           6 → 7            10 P
  QUICKLOAD          1.050s → 0.975s        10 P
                 [CONTINUE TO NEXT PHASE]
```

That board is the Gunslinger's, and it is the one on screen today. The [cast](player.md) shares this frame and the maximum-health row. Weapon rows change with the character:

| Character | Weapon rows to show | In the build |
| --- | --- | --- |
| Gunslinger | Slug capacity, quickload. Later: heat, explosive, and piercing ammunition | Yes |
| Mage | Fuel capacity, patch duration | No |
| Steamist | Canister capacity, how quickly the cross travels | No |

Use an engraved-looking heading and a readable technical face for costs and details. The current font pair is Cinzel for headings and Oxanium for controls, with earth, soot, iron, and brass UI colors. Revisit this pairing after seeing it at game resolution.
