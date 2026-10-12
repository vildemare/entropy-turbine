//! 1000 units is 1 metre, and 1 second.
//!
//! Definitions store that integer. [`units`] is the step into a world
//! transform or a frame clock, which still move in metres and seconds.

pub const fn units(value: u32) -> f32 {
    value as f32 / 1000.0
}
