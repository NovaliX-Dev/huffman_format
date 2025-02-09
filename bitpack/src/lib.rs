#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

fn u8_mask(s: u32) -> u8 {
    1u8.checked_shl(s).unwrap_or(0).wrapping_sub(1)
}

pub mod compact;

mod read;
mod write;

use cfg_if::cfg_if;
pub use read::*;
pub use write::*;

cfg_if!( if #[cfg(feature = "test_framework")] {
    pub mod test;
} else {
    #[cfg(test)]
    mod test;
});
