use crate::{WINDOW_SIZE_RANGE, prelude::*};

/// The frame header has a variable size, with a minimum of 2 bytes up to a
/// maximum of 14 bytes depending on optional parameters.
///
/// https://www.rfc-editor.org/rfc/rfc8878.pdf#name-frame-header
pub struct Header {
    descriptor: HeaderDescriptor,
    window_descriptor: WindowDescriptor,
    dictionary_id: Option<u32>,
    content_size: Option<u64>,
}

impl Header {
    pub fn read(src: &mut impl rzstd_io::Reader) -> Result<Self, Error> {
        let descriptor = HeaderDescriptor::read(src.read_u8()?)?;

        let window_descriptor = if descriptor.is_single_segment() {
            WindowDescriptor(0)
        } else {
            WindowDescriptor(src.read_u8()?)
        };

        let dictionary_id = match descriptor.did_field_size() {
            DIDFieldSize::Zero => None,
            size => {
                let mut buf = [0u8; 4];
                src.read_exact(&mut buf[..size.as_usize()])?;
                Some(u32::from_le_bytes(buf))
            }
        };

        let content_size = match descriptor.fcs_field_size() {
            FCSFieldSize::Zero => None,
            size => {
                let mut buf = [0u8; 8];
                src.read_exact(&mut buf[..size.as_usize()])?;
                Some(u64::from_le_bytes(buf) + size.offset())
            }
        };

        let header = Self {
            descriptor,
            window_descriptor,
            dictionary_id,
            content_size,
        };
        if header.descriptor.is_single_segment() {
            assert!(
                header.content_size.is_some(),
                "Single segment implies Content Size is present"
            )
        }

        Ok(header)
    }

    /// The original (uncompressed) size.
    pub fn content_size(&self) -> Option<u64> {
        self.content_size
    }

    /// The ID of the dictionary required to properly decode the frame. When
    /// it's not present, it's up to the decoder to know which dictionary to
    /// use.
    pub fn dictionary_id(&self) -> Option<u32> {
        self.dictionary_id
    }

    /// Minimum memory buffer size to to decode compressed data.
    pub fn window_size(&self) -> Result<u64, Error> {
        if self.descriptor.is_single_segment() {
            assert!(
                self.content_size.is_some(),
                "Single Segment implies Content Size is present"
            );
            return Ok(self.content_size().unwrap());
        }

        let size = self.window_descriptor.size();
        if !WINDOW_SIZE_RANGE.contains(&size) {
            return Err(Error::WindowSizeOutOfBounds(size));
        }

        Ok(size)
    }

    /// Whether the frame contains a 32-bit checksum at the end.
    pub fn has_checksum(&self) -> bool {
        self.descriptor.content_checksum_flag() == 1
    }
}

/// The first header's byte is called the [HeaderDescriptor]. It describes which
/// other fields are present. Decoding this byte is enough to tell the size of
/// [Header].
///
/// | Bit Number | Field Name              |
/// |------------|-------------------------|
/// | 7-6        | Frame_Content_Size_Flag |
/// | 5          | Single_Segment_Flag     |
/// | 4          | (unused)                |
/// | 3          | (reserved)              |
/// | 2          | Content_Checksum_Flag   |
/// | 1-0        | Dictionary_ID_Flag      |
///
/// (bit 7 is the highest bit, while bit 0 is the lowest one.)
struct HeaderDescriptor(u8);

impl HeaderDescriptor {
    fn read(val: u8) -> Result<Self, Error> {
        let ret = Self(val);

        if ret.reserved_bit() != 0 {
            return Err(Error::ReservedBitSet);
        }

        Ok(ret)
    }

    /// A 2-bit flag, specifying whether the [Header::content_size()]
    /// (decompressed data size) is provided within the header.
    fn fcs_flag(&self) -> TwoBitFlag {
        TwoBitFlag::from_u8(self.0 >> 6)
    }

    /// The number of bytes used by [Header::content_size()], derived from
    /// [HeaderDescriptor::fcs_flag()] and
    /// [HeaderDescriptor::single_segment_flag()].
    ///
    /// | fcs_flag       | 0      | 1 | 2 | 3 |
    /// |----------------|--------|---|---|---|
    /// | fcs_field_size | 0 or 1 | 2 | 4 | 8 |
    ///
    /// When [HeaderDescriptor::fcs_flag()] is 0
    /// [HeaderDescriptor::fcs_field_size()] depends on
    /// [HeaderDescriptor::single_segment_flag()]; if it is set,
    /// [HeaderDescriptor::fcs_field_size()] is 1. Otherwise,
    /// [HeaderDescriptor::fcs_field_size()] is 0, and [Header::content_size()]
    /// is not provided.
    fn fcs_field_size(&self) -> FCSFieldSize {
        match self.fcs_flag() {
            TwoBitFlag::Zero => {
                if !self.is_single_segment() {
                    FCSFieldSize::Zero
                } else {
                    FCSFieldSize::One
                }
            }
            TwoBitFlag::One => FCSFieldSize::Two,
            TwoBitFlag::Two => FCSFieldSize::Four,
            TwoBitFlag::Three => FCSFieldSize::Eight,
        }
    }

    /// A bit flag, specifying whether data must be regenerated within a single
    /// continuous memory segment.
    ///
    /// In this case, [WindowDescriptor] is skipped, but
    /// [Header::content_size()] is necessarily present. As a consequence,
    /// the decoder must allocate a memory segment of a size equal to or
    /// larger than [Header::content_size()].
    const fn single_segment_flag(&self) -> u8 {
        let val = (self.0 & 0x20) >> 5;
        assert!(val == 0 || val == 1, "Invalid single segment flag");
        val
    }

    fn is_single_segment(&self) -> bool {
        self.single_segment_flag() == 1
    }

    fn reserved_bit(&self) -> u8 {
        (self.0 & 0x8) >> 3
    }

    /// A bit flag, specifying whether a 32-bit [ContentChecksum] will be
    /// present at the frame's end.
    fn content_checksum_flag(&self) -> u8 {
        (self.0 & 0x04) >> 2
    }

    /// A 2-bit flag, indicating whether a dictionary ID is provided within the
    /// header. It also specifies the size of this field
    fn dictionary_id_flag(&self) -> TwoBitFlag {
        TwoBitFlag::from_u8(self.0 & 0x03)
    }

    /// The number of bytes used by [Header::dictionary_id()], derived from
    /// [HeaderDescriptor::dictionary_id_flag()].
    ///
    /// | dictionary_id  | 0 | 1 | 2 | 3 |
    /// |----------------|---|---|---|---|
    /// | did_field_size | 0 | 1 | 2 | 4 |
    fn did_field_size(&self) -> DIDFieldSize {
        match self.dictionary_id_flag() {
            TwoBitFlag::Zero => DIDFieldSize::Zero,
            TwoBitFlag::One => DIDFieldSize::One,
            TwoBitFlag::Two => DIDFieldSize::Two,
            TwoBitFlag::Three => DIDFieldSize::Four,
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum FCSFieldSize {
    Zero = 0,
    One = 1,
    Two = 2,
    Four = 4,
    Eight = 8,
}

const_assert!(FCSFieldSize::Zero as usize == 0);
const_assert!(FCSFieldSize::One as usize == 1);
const_assert!(FCSFieldSize::Two as usize == 2);
const_assert!(FCSFieldSize::Four as usize == 4);
const_assert!(FCSFieldSize::Eight as usize == 8);

impl FCSFieldSize {
    const fn as_usize(self) -> usize {
        self as usize
    }

    /// Calculate the offset to add to the read value.
    const fn offset(&self) -> u64 {
        match self {
            Self::Two => 256,
            _ => 0,
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum DIDFieldSize {
    Zero = 0,
    One = 1,
    Two = 2,
    Four = 4,
}

const_assert!(DIDFieldSize::Zero as usize == 0);
const_assert!(DIDFieldSize::One as usize == 1);
const_assert!(DIDFieldSize::Two as usize == 2);
const_assert!(DIDFieldSize::Four as usize == 4);

impl DIDFieldSize {
    pub const fn as_usize(self) -> usize {
        self as usize
    }
}

/// This provides guarantees about the minimum memory buffer required to
/// decompress a frame.
/// This information is important for decoders to allocate enough memory.
///
/// | Bit number | 7-3      | 2-0      |
/// |------------|----------|----------|
/// | Field name | exponent | mantissa |
struct WindowDescriptor(u8);

impl WindowDescriptor {
    const fn exponent(&self) -> u8 {
        let val = self.0 >> 3;
        assert!(val < 0x20, "Exponent is 5 bits");
        val
    }

    const fn mantissa(&self) -> u8 {
        let val = self.0 & 0x7;
        assert!(val < 0x8, "Mantissa is 3 bits");
        val
    }

    const fn size(&self) -> u64 {
        let window_log = 10 + self.exponent() as u64;
        let window_base = 1 << window_log;
        let window_add = (window_base >> 3) * self.mantissa() as u64;
        window_base + window_add
    }
}
