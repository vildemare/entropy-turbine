# Audio

Part of the [game design](game_design.md).

Keep effects short and readable in a dense fight: a low electrical thump for player plasma, dry mechanical crack for enemy guns, metal impact against cover, and a heavier warning for player damage. The current WAV set is synthesized locally and can be regenerated with `tools/generate_sfx.py`. Player and enemy gunfire each randomly select from three lower-pitched tonal variants to soften repetition. Keep using WAVs while exploring the sound palette; later, define sound profiles in Rust when weapons and enemy capabilities need sounds that react to their properties. Avoid a constant loud soundtrack; leave room for shot timing and a low atmospheric bed. Revisit levels and sound design after hands-on playtests.

The Gunslinger's plasma thump is the player weapon in the build. The Mage's cast and the Steamist's charge and burst need their own short cues when those weapons exist: a fuel ignition for the patch, a rising pressure tick while the Steamist holds, and a steam crack when the canister lands. Those cues are not recorded yet. They should stay as short as the plasma thump so a dense fight can still be read.
