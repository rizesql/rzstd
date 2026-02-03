use crate::{MAX_BLOCK_SIZE, context::Context, prelude::*};

const RAW_RLE_BUF_SIZE: [Option<usize>; 4] = [None, Some(1), None, Some(2)];
const RAW_RLE_SHIFT: [usize; 4] = [3, 4, 3, 4];
const COMPRESSED_BUF_SIZE: [usize; 4] = [2, 2, 3, 4];
const COMPRESSED_BITS: [usize; 4] = [10, 10, 14, 18];
const COMPRESSED_STREAMS: [Streams; 4] =
    [Streams::One, Streams::Four, Streams::Four, Streams::Four];

impl<R: rzstd_io::Reader> Context<'_, R> {
    pub fn literals_section(&mut self) -> Result<(), Error> {
        let header = Header::read(&mut self.src)?;
        if header.regenerated_size > MAX_BLOCK_SIZE {
            return Err(Error::LiteralsSizeTooLarge(header.regenerated_size));
        }

        let dst = &mut self.literals_buf[..header.regenerated_size as usize];

        match header.ls_type {
            Type::Raw => self.src.read_exact(dst).map_err(Error::from),

            Type::RLE => {
                let byte = self.src.read_u8()?;
                dst.fill(byte);
                Ok(())
            }

            Type::Compressed | Type::Treeless => {
                let compressed_size =
                    header.compressed_size.ok_or(Error::MissingCompressedSize)?;
                if compressed_size > MAX_BLOCK_SIZE {
                    return Err(Error::CompressedSizeTooLarge(compressed_size));
                }

                let scratch = &mut self.scratch_buf[..compressed_size as usize];
                self.src.read_exact(scratch)?;

                let read = if header.ls_type == Type::Compressed {
                    let (table, read) = rzstd_huff0::DecodingTable::read(scratch)?;
                    self.huff.table = Some(table);
                    read
                } else {
                    0
                };

                let table = self.huff.table.as_ref().ok_or(Error::MissingHuffTable)?;
                Self::huff_streams(&scratch[read..], dst, table, header.streams)
            }
        }
    }

    fn huff_streams(
        src: &[u8],
        dst: &mut [u8],
        table: &rzstd_huff0::DecodingTable,
        streams: Streams,
    ) -> Result<(), Error> {
        let decoder = rzstd_huff0::Decoder::new(table);

        match streams {
            Streams::One => {
                let mut r = rzstd_io::ReverseBitReader::new(src)?;

                for d in dst.iter_mut() {
                    *d = decoder.decode(&mut r)?;
                }

                if r.bits_remaining() > 0 {
                    return Err(Error::ExtraBitsInStream(r.bits_remaining()));
                }

                Ok(())
            }
            Streams::Four => {
                if src.len() < 6 {
                    return Err(Error::JumpTableError(
                        "Source too short for jump table".into(),
                    ));
                }

                let mut readers = {
                    let s0 = u16::from_le_bytes([src[0], src[1]]) as usize;
                    let s1 = u16::from_le_bytes([src[2], src[3]]) as usize;
                    let s2 = u16::from_le_bytes([src[4], src[5]]) as usize;

                    let total = 6 + s0 + s1 + s2;
                    if total >= src.len() {
                        return Err(Error::JumpTableError(
                            "Jump table offsets exceed source length".into(),
                        ));
                    }

                    [
                        rzstd_io::ReverseBitReader::new(&src[6..6 + s0])?,
                        rzstd_io::ReverseBitReader::new(&src[6 + s0..6 + s0 + s1])?,
                        rzstd_io::ReverseBitReader::new(&src[6 + s0 + s1..total])?,
                        rzstd_io::ReverseBitReader::new(&src[total..])?,
                    ]
                };

                let chunk = (dst.len() + 3) / 4;
                if dst.len() < 3 * chunk {
                    return Err(Error::LiteralsBufferTooSmall);
                }

                let (out0, rem) = dst.split_at_mut(chunk);
                let (out1, rem) = rem.split_at_mut(chunk);
                let (out2, out3) = rem.split_at_mut(chunk);

                let batch = 4;
                let n_batches = out3.len() / batch;

                for b in 0..n_batches {
                    let offset = b * batch;

                    let s0 = decoder.decode4(&mut readers[0])?;
                    for i in 0..4 {
                        out0[offset + i] = s0[i];
                    }

                    let s1 = decoder.decode4(&mut readers[1])?;
                    for i in 0..4 {
                        out1[offset + i] = s1[i];
                    }

                    let s2 = decoder.decode4(&mut readers[2])?;
                    for i in 0..4 {
                        out2[offset + i] = s2[i];
                    }

                    let s3 = decoder.decode4(&mut readers[3])?;
                    for i in 0..4 {
                        out3[offset + i] = s3[i];
                    }

                    // out0[offset..offset + 4].copy_from_slice(&s0);
                    // out1[offset..offset + 4].copy_from_slice(&s1);
                    // out2[offset..offset + 4].copy_from_slice(&s2);
                    // out3[offset..offset + 4].copy_from_slice(&s3);
                }

                for i in (n_batches * batch)..out3.len() {
                    out0[i] = decoder.decode(&mut readers[0])?;
                    out1[i] = decoder.decode(&mut readers[1])?;
                    out2[i] = decoder.decode(&mut readers[2])?;
                    out3[i] = decoder.decode(&mut readers[3])?;
                }

                for i in out3.len()..chunk {
                    if let Some(d0) = out0.get_mut(i) {
                        *d0 = decoder.decode(&mut readers[0])?;
                    }
                    if let Some(d1) = out1.get_mut(i) {
                        *d1 = decoder.decode(&mut readers[1])?;
                    }
                    if let Some(d2) = out2.get_mut(i) {
                        *d2 = decoder.decode(&mut readers[2])?;
                    }
                }

                for r in &readers {
                    if r.bits_remaining() > 0 {
                        return Err(Error::ExtraBitsInStream(r.bits_remaining()));
                    }
                }

                Ok(())
            }
        }
    }
}

pub struct Header {
    ls_type: Type,
    regenerated_size: u32,
    compressed_size: Option<u32>,
    streams: Streams,
}

impl Header {
    pub fn read(r: &mut impl rzstd_io::Reader) -> Result<Header, Error> {
        let first = r.read_u8()?;

        let ls_type = Type::from(TwoBitFlag::from_u8(first & 0x03));
        let size_format = TwoBitFlag::from_u8((first >> 2) & 0x3);

        match ls_type {
            Type::Raw | Type::RLE => Self::read_raw_rle(r, first, ls_type, size_format),
            Type::Compressed | Type::Treeless => {
                Self::read_compressed(r, first, ls_type, size_format)
            }
        }
    }

    fn read_raw_rle(
        r: &mut impl rzstd_io::Reader,
        first: u8,
        ls_type: Type,
        size_format: TwoBitFlag,
    ) -> Result<Header, Error> {
        assert!(matches!(ls_type, Type::Raw | Type::RLE));

        let buf_size = RAW_RLE_BUF_SIZE[size_format as usize];
        let header = {
            let mut buf = [0u8; 4];
            buf[0] = first;

            if let Some(buf_size) = buf_size {
                r.read_exact(&mut buf[1..=buf_size])?;
            }

            u32::from_le_bytes(buf)
        };

        let shift = RAW_RLE_SHIFT[size_format as usize];
        let regenerated_size = header >> shift;

        Ok(Self {
            ls_type,
            regenerated_size,
            streams: Streams::One,
            compressed_size: None,
        })
    }

    fn read_compressed(
        r: &mut impl rzstd_io::Reader,
        first: u8,
        ls_type: Type,
        size_format: TwoBitFlag,
    ) -> Result<Header, Error> {
        assert!(matches!(ls_type, Type::Compressed | Type::Treeless));

        let buf_size = COMPRESSED_BUF_SIZE[size_format as usize];
        let n_bits = COMPRESSED_BITS[size_format as usize];
        let mask = (1 << n_bits) - 1;

        let header = {
            let mut buf = [0u8; 8];
            buf[0] = first;
            r.read_exact(&mut buf[1..=buf_size])?;
            u64::from_le_bytes(buf)
        };
        let header = header >> 4;

        let regenerated_size = (header & mask) as u32;
        let compressed_size = ((header >> n_bits) & mask) as u32;

        Ok(Self {
            ls_type,
            regenerated_size,
            streams: COMPRESSED_STREAMS[size_format as usize],
            compressed_size: Some(compressed_size),
        })
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Streams {
    One = 1,
    Four = 4,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Type {
    /// Literals are stored uncompressed
    Raw = 0,

    /// Literals consist of a single-byte value repeated
    /// [header::regenerated_size()] times
    RLE = 1,

    /// A standard Huffman-compressed block, starting with a Huffman tree
    /// description.
    Compressed = 2,

    /// A Huffman-compressed block, using the Huffman tree from the
    /// previous [Type::Compressed] block or a dictionary if there is no
    /// previous Huffman- compressed literals block.
    /// If this mode is triggered without any previous Huffman table in the
    /// frame (or dictionary), it should be treated as data corruption.
    Treeless = 3,
}

impl From<TwoBitFlag> for Type {
    fn from(value: TwoBitFlag) -> Self {
        match value {
            TwoBitFlag::Zero => Self::Raw,
            TwoBitFlag::One => Self::RLE,
            TwoBitFlag::Two => Self::Compressed,
            TwoBitFlag::Three => Self::Treeless,
        }
    }
}
