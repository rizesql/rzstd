use crate::{MAX_BLOCK_SIZE, context::Context, prelude::*};

pub const HEADER_SIZE: usize = 3;

impl<R: rzstd_io::Reader> Context<'_, R> {
    pub fn block(&mut self) -> Result<bool, Error> {
        let header = Header::read(&mut self.src)?;
        match header.block_type() {
            Type::Raw => {
                let count = header.decompressed_size().ok_or(Error::MissingBlockSize)?;
                self.window_buf.read_from(&mut self.src, count as usize)?;
            }
            Type::RLE => {
                let count = header.decompressed_size().ok_or(Error::MissingBlockSize)?;
                let byte = self.src.read_u8()?;
                self.window_buf.push_rle(byte, count as usize);
            }
            Type::Compressed => {
                self.literals_section()?;
                self.execute_sequences(header.content_size())?;
            }
        }

        Ok(header.last_block())
    }
}

/// The Block Header contains information about the block type and size.
pub struct Header {
    last_block: bool,
    block_type: Type,
    block_size: u32,
}

impl Header {
    pub fn read(r: &mut impl rzstd_io::Reader) -> Result<Self, Error> {
        let raw = {
            let mut buf = [0u8; 4];
            r.read_exact(&mut buf[..HEADER_SIZE])?;
            u32::from_le_bytes(buf)
        };

        let last_block = raw & 0x01 != 0;

        let block_type = {
            let block_type = ((raw >> 1) & 0x03) as u8;
            Type::try_from(TwoBitFlag::from_u8(block_type))?
        };

        let block_size = raw >> 3;
        if block_size > MAX_BLOCK_SIZE {
            return Err(Error::BlockSizeOutOfBounds(block_size));
        }

        assert!(
            block_size <= MAX_BLOCK_SIZE,
            "Block size exceeds maximum allowed"
        );
        Ok(Self {
            last_block,
            block_type,
            block_size,
        })
    }

    ///  Signals whether this block is the last one. The frame will end after
    /// this last block. It may be followed by an optional [TODO
    /// ContentChecksum]
    pub fn last_block(&self) -> bool {
        self.last_block
    }

    pub fn block_type(&self) -> Type {
        self.block_type
    }

    pub fn decompressed_size(&self) -> Option<u32> {
        match self.block_type {
            Type::Raw | Type::RLE => Some(self.block_size),
            Type::Compressed => None,
        }
    }

    pub fn content_size(&self) -> u32 {
        match self.block_type {
            Type::RLE => 1,
            Type::Raw | Type::Compressed => self.block_size,
        }
    }
}

#[derive(Clone, Copy, Debug)]
/// The type of the block
pub enum Type {
    /// An uncompressed block. [TODO BlockContent] contains
    /// [Header::block_size()] bytes.
    Raw = 0,

    /// A single byte, repeated [Header::block_size()] times. [TODO
    /// BlockContent] consists of a single byte. On the decompression side,
    /// this byte must be repeated [Header::block_size()] times.
    RLE = 1,

    /// A compressed block. [Header::block_size()] is the length of [TODO
    /// BlockContent], namely the compressed data. The decompressed size is not
    /// known, but it's maximum possible value is guaranteed.
    Compressed = 2,
}

const_assert!(Type::Raw.as_u32() == 0);
const_assert!(Type::RLE.as_u32() == 1);
const_assert!(Type::Compressed.as_u32() == 2);

impl Type {
    pub const fn as_u32(self) -> u32 {
        self as u32
    }
}

impl TryFrom<TwoBitFlag> for Type {
    type Error = Error;

    fn try_from(flag: TwoBitFlag) -> Result<Self, Self::Error> {
        match flag {
            TwoBitFlag::Zero => Ok(Self::Raw),
            TwoBitFlag::One => Ok(Self::RLE),
            TwoBitFlag::Two => Ok(Self::Compressed),
            TwoBitFlag::Three => Err(Error::ReservedBlock),
        }
    }
}
