# Players

Part of the [game design](game_design.md).

Plan for four selectable characters with distinct weapons and upgrade paths. Three are specified:

| Character | Status | Range | Shot | How it is aimed |
| --- | --- | --- | --- | --- |
| [Gunslinger](players/gunslinger.md) | Playable | Long, direct | Paired plasma rounds | Mouse direction |
| [Mage](players/mage.md) | Designed | Short | Lingering fire from fuel tanks | Mouse direction, distance capped |
| [Steamist](players/steamist.md) | Designed | Short, indirect | Exploding canister | Same mouse aim. The cross walks toward the pointer and stops on it |
| Fourth | Open | | | |

Do not invent the fourth role to fill the roster. Add it when its weapon, and the way the player aims that weapon, are as clear as these three.

Every character can upgrade maximum health. Avoid adding a shared energy pool unless a later ability genuinely needs one. Health, ammunition, and points are enough. Fuel in the Mage's tanks and canisters in the Steamist's launcher are that character's ammunition, not a pool the others draw from.

Holding the fire button slows movement and the run animation to two thirds speed while the weapon is using that hold: firing, casting, or building pressure. The Gunslinger's quickloader is the current exception, described on that character's page.

Purchased levels form the player build. Derive maximum health and the weapon's limits from that build whenever it changes and at phase start. Keep current health and the weapon's live timers as combat state. Continuing from a checkpoint restores health and refills the weapon.

The [checkpoint shop](ui.md) shows one character at a time. The roster shares the menu frame and the health row. The weapon rows change with the character. Only the Gunslinger's rows are in the current build.
