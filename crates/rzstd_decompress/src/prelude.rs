pub use rzstd_foundation::*;

pub use crate::errors::*;

#[derive(Debug, Clone, Copy)]
pub enum TwoBitFlag {
    Zero = 0,
    One = 1,
    Two = 2,
    Three = 3,
}

impl TwoBitFlag {
    /// It is expected at the caller site to truncate the input so it's < 4
    pub fn from_u8(val: u8) -> TwoBitFlag {
        assert!(val < 4, "Invalid value for TwoBitFlag");
        match val {
            0 => TwoBitFlag::Zero,
            1 => TwoBitFlag::One,
            2 => TwoBitFlag::Two,
            3 => TwoBitFlag::Three,
            _ => unreachable!("Invalid value for TwoBitFlag"),
        }
    }
}
