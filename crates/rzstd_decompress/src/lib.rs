use rzstd_foundation::const_assert;

mod block;
mod context;
mod decoder;
mod errors;
mod frame;
mod literals_section;
mod prelude;
mod sequence_execution;
mod sequences_section;
mod window;

pub use decoder::Decoder;
pub use errors::Error;

pub const MAGIC_NUM: u32 = 0xFD2F_B528;

pub const MIN_WINDOW_SIZE: u64 = 0x400;
pub const MAX_WINDOW_SIZE: u64 = 128 * 1024 * 1024;
pub const WINDOW_SIZE_RANGE: std::ops::RangeInclusive<u64> =
    MIN_WINDOW_SIZE..=MAX_WINDOW_SIZE;

pub const MAX_BLOCK_SIZE: u32 = 128 * 1024;

pub const LL_DIST: DefaultDistribution = DefaultDistribution {
    accuracy_log: 9,
    predefined_accuracy_log: 6,
    predefined_table: &[
        4, 3, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 1, 1, 1, 2, 2, 2, 2, 2, 2, 2, 2, 2, 3, 2,
        1, 1, 1, 1, 1, -1, -1, -1, -1,
    ],
};
const_assert!(LL_DIST.predefined_table().len() == 36);

pub const ML_DIST: DefaultDistribution = DefaultDistribution {
    accuracy_log: 9,
    predefined_accuracy_log: 6,
    predefined_table: &[
        1, 4, 3, 2, 2, 2, 2, 2, 2, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
        1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, -1, -1, -1, -1, -1, -1,
        -1,
    ],
};
const_assert!(ML_DIST.predefined_table().len() == 53);

pub const OF_DIST: DefaultDistribution = DefaultDistribution {
    accuracy_log: 8,
    predefined_accuracy_log: 5,
    predefined_table: &[
        1, 1, 1, 1, 1, 1, 2, 2, 2, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, -1, -1,
        -1, -1, -1,
    ],
};
const_assert!(OF_DIST.predefined_table().len() == 29);

pub struct DefaultDistribution {
    accuracy_log: usize,
    predefined_accuracy_log: usize,
    predefined_table: &'static [i16],
}

impl DefaultDistribution {
    pub const fn accuracy_log(&self) -> usize {
        self.predefined_accuracy_log
    }

    pub const fn table_size(&self) -> usize {
        1 << self.accuracy_log
    }

    pub const fn predefined_table(&self) -> &'static [i16] {
        &self.predefined_table
    }
}
