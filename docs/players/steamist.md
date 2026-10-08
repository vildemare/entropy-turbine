# Steamist

Part of the [player cast](../player.md). Designed, not in the current build. Build this after the [Gunslinger](gunslinger.md) slice stays readable.

The Steamist ejects small exploding canisters. The defined fill is hyper-critical water, hot enough to flash into steam when the canister bursts. A later mix of another liquid can change that burst. The first canister is water only. Do not add a mix until the arc and the landing marker are readable.

The shot is short-ranged and indirect. It does not fly in a straight line. He aims with the same mouse point as the other characters, a spot on the ground plane. The canister comes down on the cross, which is chasing that spot.

## Pressure

Holding the fire button builds pressure. Releasing it launches one canister. A tap is the shortest lob, so a hurried shot stays near his feet. There is no separate cancel. Releasing the button fires.

Pressure exists only during that hold. It is not stored afterward, and it is not an energy pool. Canisters in the launcher are the ammunition. Continuing from a checkpoint refills them.

Holding the button slows movement to two thirds, the same cost as firing does for the others. A long charge in the open has to be worth the marker.

## Landing cross

The mouse pointer is the place he wants the canister to land. On the press, a cross appears at the Steamist and starts moving toward that point. The cross alternates black and red. Where it sits is where the canister would land if he released now.

The cross never goes past the mouse point. When it reaches the pointer, it stays there. Pressure can keep rising after that. The landing spot does not move any farther. The extra pressure only makes the arc higher, so the same landing clears a taller barricade.

Moving the mouse during the hold moves the point the cross is chasing. If the pointer comes closer than the cross, the cross stays on the pointer instead of hanging past it. On release the cross disappears and the canister travels to the spot the cross was showing.

The flight does not hit actors or cover. Straight shots still stop on barricades, as in [Combat](../combat.md). The burst at the cross is what damages. The burst is immediate and small, smaller than the Mage's lingering patch. It does not stay on the ground. Lingering denial belongs to the [Mage](mage.md). The Steamist owns one placed explosion.

## Shop

The Steamist uses the same [shop](../ui.md) frame, maximum-health row, and price ladder as the others. The first weapon rows to try are canister capacity and how quickly the cross travels. Blast radius waits until the landing cross is trustworthy.
