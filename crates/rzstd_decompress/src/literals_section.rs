use crate::{MAX_BLOCK_SIZE, context::Context, prelude::*};

const RAW_RLE_BUF_SIZE: [Option<usize>; 4] = [None, Some(1), None, Some(2)];
const RAW_RLE_SHIFT: [usize; 4] = [3, 4, 3, 4];
const COMPRESSED_BUF_SIZE: [usize; 4] = [2, 2, 3, 4];
const COMPRESSED_BITS: [usize; 4] = [10, 10, 14, 18];
const COMPRESSED_STREAMS: [Streams; 4] =
    [Streams::One, Streams::Four, Streams::Four, Streams::Four];

impl<R: rzstd_io::Reader> Context<'_, R> {
    pub fn literals_section(&mut self) -> Result<u32, Error> {
        let (header, read) = Header::read(&mut self.src)?;
        if header.regenerated_size > MAX_BLOCK_SIZE {
            return Err(Error::LiteralsSizeTooLarge(header.regenerated_size));
        }

        tracing::debug!("literals section header={:?}\n", header);

        let literals_size = match header.compressed_size {
            Some(it) => it,
            None => match header.ls_type {
                Type::RLE => 1,
                Type::Raw => header.regenerated_size,
                Type::Compressed | Type::Treeless => unreachable!(),
            },
        } as usize;

        let dst = &mut self.literals_buf[..header.regenerated_size as usize];
        self.literals_idx = header.regenerated_size as usize;
        match header.ls_type {
            Type::Raw => {
                self.src.read_exact(dst).map_err(Error::from)?;
            }

            Type::RLE => {
                let byte = self.src.read_u8()?;
                dst.fill(byte);
            }

            Type::Compressed | Type::Treeless => {
                let compressed_size =
                    header.compressed_size.ok_or(Error::MissingCompressedSize)?;
                if compressed_size > MAX_BLOCK_SIZE {
                    return Err(Error::CompressedSizeTooLarge(compressed_size));
                }

                let scratch = &mut self.scratch_buf[..literals_size as usize];
                self.src.read_exact(scratch)?;

                let read = if header.ls_type == Type::Compressed {
                    let (table, read) = rzstd_huff0::DecodingTable::read(scratch)?;
                    self.huff.table = Some(table);
                    read
                } else {
                    0
                };

                let table = self.huff.table.as_ref().ok_or(Error::MissingHuffTable)?;
                Self::huff_streams(&scratch[read..], dst, table, header.streams)?;
            }
        };
        Ok((literals_size + read) as u32)
    }

    fn huff_streams(
        src: &[u8],
        dst: &mut [u8],
        table: &rzstd_huff0::DecodingTable,
        streams: Streams,
    ) -> Result<(), Error> {
        match streams {
            Streams::One => {
                let mut r = rzstd_io::ReverseBitReader::new(src)?;
                let mut decoder = rzstd_huff0::Decoder::new(table, &mut r);

                for d in dst.iter_mut() {
                    *d = decoder.decode(&mut r);
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
                    let s0 = src[0] as usize + ((src[1] as usize) << 8);
                    let s1 = s0 + src[2] as usize + ((src[3] as usize) << 8);
                    let s2 = s1 + src[4] as usize + ((src[5] as usize) << 8);

                    if s2 > src.len() {
                        return Err(Error::JumpTableError(
                            "Jump table offsets exceed source length".into(),
                        ));
                    }

                    let src = &src[6..];

                    [
                        rzstd_io::ReverseBitReader::new(&src[..s0])?,
                        rzstd_io::ReverseBitReader::new(&src[s0..s1])?,
                        rzstd_io::ReverseBitReader::new(&src[s1..s2])?,
                        rzstd_io::ReverseBitReader::new(&src[s2..])?,
                    ]
                };

                let chunk = (dst.len() + 3) / 4;
                let last_chunk_size = dst.len() - (chunk * 3);

                if dst.len() < 3 * chunk {
                    return Err(Error::LiteralsBufferTooSmall);
                }

                let (out0, rem) = dst.split_at_mut(chunk);
                let (out1, rem) = rem.split_at_mut(chunk);
                let (out2, out3) = rem.split_at_mut(chunk);

                let mut decoder0 = rzstd_huff0::Decoder::new(table, &mut readers[0]);
                let mut decoder1 = rzstd_huff0::Decoder::new(table, &mut readers[1]);
                let mut decoder2 = rzstd_huff0::Decoder::new(table, &mut readers[2]);
                let mut decoder3 = rzstd_huff0::Decoder::new(table, &mut readers[3]);

                let burst_len = chunk.min(last_chunk_size);
                for i in 0..burst_len {
                    out0[i] = decoder0.decode(&mut readers[0]);
                    out1[i] = decoder1.decode(&mut readers[1]);
                    out2[i] = decoder2.decode(&mut readers[2]);
                    out3[i] = decoder3.decode(&mut readers[3]);
                }

                if chunk > burst_len {
                    for i in burst_len..chunk {
                        out0[i] = decoder0.decode(&mut readers[0]);
                        out1[i] = decoder1.decode(&mut readers[1]);
                        out2[i] = decoder2.decode(&mut readers[2]);
                    }
                }

                for r in readers.iter() {
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

impl std::fmt::Debug for Header {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LiteralsSection")
            .field("ls_type", &self.ls_type)
            .field("regenerated_size", &self.regenerated_size)
            .field("compressed_size", &self.compressed_size)
            .field(
                "num_streams",
                match self.streams {
                    Streams::One => &Some(1),
                    Streams::Four => &Some(4),
                },
            )
            .finish()
    }
}

impl Header {
    pub fn read(src: &mut impl rzstd_io::Reader) -> Result<(Header, usize), Error> {
        let first = src.read_u8()?;

        let ls_type = Type::from(TwoBitFlag::from_u8(first & 0x03));
        let size_format = TwoBitFlag::from_u8((first >> 2) & 0x3);

        match ls_type {
            Type::Raw | Type::RLE => {
                let (header, n_bytes) =
                    Self::read_raw_rle(src, first, ls_type, size_format)?;
                Ok((header, n_bytes + 1))
            }
            Type::Compressed | Type::Treeless => {
                let (header, n_bytes) =
                    Self::read_compressed(src, first, ls_type, size_format)?;
                Ok((header, n_bytes + 1))
            }
        }
    }

    fn read_raw_rle(
        mut src: impl rzstd_io::Reader,
        first: u8,
        ls_type: Type,
        size_format: TwoBitFlag,
    ) -> Result<(Header, usize), Error> {
        assert!(matches!(ls_type, Type::Raw | Type::RLE));

        let buf_size = RAW_RLE_BUF_SIZE[size_format as usize];
        let header = {
            let mut buf = [0u8; 4];
            buf[0] = first;

            if let Some(buf_size) = buf_size {
                src.read_exact(&mut buf[1..=buf_size])?;
            }

            u32::from_le_bytes(buf)
        };

        let shift = RAW_RLE_SHIFT[size_format as usize];
        let regenerated_size = header >> shift;

        Ok((
            Self {
                ls_type,
                regenerated_size,
                streams: Streams::One,
                compressed_size: None,
            },
            buf_size.unwrap_or(0),
        ))
    }

    fn read_compressed(
        mut src: impl rzstd_io::Reader,
        first: u8,
        ls_type: Type,
        size_format: TwoBitFlag,
    ) -> Result<(Header, usize), Error> {
        assert!(matches!(ls_type, Type::Compressed | Type::Treeless));

        let buf_size = COMPRESSED_BUF_SIZE[size_format as usize];
        let n_bits = COMPRESSED_BITS[size_format as usize];
        let mask = (1 << n_bits) - 1;

        let header = {
            let mut buf = [0u8; 8];
            buf[0] = first;
            src.read_exact(&mut buf[1..=buf_size])?;
            u64::from_le_bytes(buf)
        };
        let header = header >> 4;

        let regenerated_size = (header & mask) as u32;
        let compressed_size = ((header >> n_bits) & mask) as u32;

        Ok((
            Self {
                ls_type,
                regenerated_size,
                streams: COMPRESSED_STREAMS[size_format as usize],
                compressed_size: Some(compressed_size),
            },
            buf_size,
        ))
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
