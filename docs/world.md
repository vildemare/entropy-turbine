# World

Part of the [game design](game_design.md).

The opening stretch is dirt and rock, bounded by continuous boulders. The route wiggles. A bend is about fifty metres
long, the centerline swings up to eleven metres either side of the start, and the sideways slope stays under seventy
percent. Straight stretches still separate some of the bends. The camera follows that swing sideways and yaws so its
view runs along the road. The yaw picks up from rest, brakes as it lines up, and finishes the last of the turn slowly.
It keeps its forward-only scroll. When the view slides or yaws, the pointer slides with it so the ground point under the
cursor stays put. Forward scrolling does not drag the pointer. Play captures the pointer so that sideways slide can move
it, and draws the aim mark itself. The pause menu, the shop, and the death screen release the capture. The dirt strip,
visible side boulders, player and enemy movement bounds, and encounter positions all use the same centerline. The side
boulders stop shots. The right line's inner edge meets the player movement boundary. The left line sits 50 cm inside
that edge so the road's left lip stays covered. Do not present it as a paved street. Rubble, barricades, and other cover
should fit a darker steampunk palette: earth, soot, iron, leather, and aged brass. Primitive geometry is acceptable
while game behavior is being established.

Later, allow a split where the route divides into two paths and rejoins. The ordinary path keeps the expected
difficulty; the harder path offers more points. The split should be clear early enough to choose deliberately, and both
paths must lead back into the same progression. That branch waits until its layout is designed with the camera.

A cover site is a warning to the player. Build its barricades and rubble well ahead of the view, about twice the old
lead, so a machine driving in meets the obstacle with room to turn. Spawn the defenders when that barricade reaches the
top of the screen, including while the player is running forward, so visible cover still predicts the encounter. The
central wall and both visible end blocks have matching solid bounds that stop projectiles and actors. Two smaller solid
rubble pieces between sites give the player places to break incoming fire. Defenders shelter behind the main wall, then
leave by its sides. Enemies that reach the camera's rear area stop firing and retreat toward the side rocks; they
disappear at the edge or after a short delay if blocked. Who arrives, and how they move, is in [Enemies](enemy.md). What
stops a shot is in [Combat](combat.md).

Rubble uses the low-poly `models/rubble_cluster.glb` rock formation instead of a stretched cube. Its small form is
authored at the existing 1.3 by 0.8 by 1.4 metre gameplay bounds; field cover scales that same formation to its larger
box. Each wave gives its four piles different quarter-turn orientations, and shifts those orientations on the next wave.
Collision remains the simple cover box, so the richer silhouette does not complicate movement or projectile tests.

The current player visual is a recolored, animated Toon Shooter Kit character. It is a temporary reference for later
original characters. Keep model loading and animation separate from the player's movement, collision, health, and input.
Future `.glb`/`.gltf` characters, weapons, and props should replace visuals without changing core combat rules. The
Gunslinger wears this stand-in today. The [Mage](players/mage.md) and the [Steamist](players/steamist.md) need their own
silhouettes when they are built.
