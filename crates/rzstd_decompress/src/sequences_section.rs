use crate::{
    DefaultDistribution, LL_DIST, ML_DIST, OF_DIST, context::Context, prelude::*,
};

#[derive(Clone, Copy, Default)]
pub struct Sequence {
    pub lit_len: u32,
    pub offset: u32,
    pub match_len: u32,
}

impl std::fmt::Debug for Sequence {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Sequence")
            .field("ll", &self.lit_len)
            .field("ml", &self.match_len)
            .field("of", &self.offset)
            .finish()
    }
}

impl<R: rzstd_io::Reader> Context<'_, R> {
    pub fn sequence_section(&mut self, seq_size: usize) -> Result<(), Error> {
        let scratch = &mut self.scratch_buf[..seq_size];
        self.src.read_exact(scratch)?;
        let mut reader: &[u8] = scratch;

        let header = Header::read(&mut reader)?;
        if header.n_seqs == 0 {
            return Ok(());
        }

        tracing::debug!("\nsequence section header={:?}\n", header);

        let modes = header.modes.as_ref().ok_or(Error::MissingModes)?;

        let mut idx = 0;

        tracing::debug!("updating ll mode={:?}", modes.literal_lengths());
        idx += update_table(
            modes.literal_lengths(),
            LL_DIST,
            &reader[idx..],
            &mut self.fse.ll,
        )?;
        tracing::debug!(
            "ll_table.len={:?}; ll_table={:?}",
            self.fse.ll.as_ref().unwrap().table().len(),
            self.fse.ll.as_ref().unwrap().table(),
        );

        tracing::debug!("\nupdating of mode={:?}", modes.offsets());
        idx += update_table(modes.offsets(), OF_DIST, &reader[idx..], &mut self.fse.of)?;
        tracing::debug!(
            "of_table.len={:?}; of_table={:?}",
            self.fse.of.as_ref().unwrap().table().len(),
            self.fse.of.as_ref().unwrap().table(),
        );

        tracing::debug!("\nupdating ml mode={:?}", modes.match_lengths());
        idx += update_table(
            modes.match_lengths(),
            ML_DIST,
            &reader[idx..],
            &mut self.fse.ml,
        )?;
        tracing::debug!(
            "ml_table.len={:?}; ml_table={:?}\n",
            self.fse.ml.as_ref().unwrap().table().len(),
            self.fse.ml.as_ref().unwrap().table(),
        );

        let mut r = rzstd_io::ReverseBitReader::new(&reader[idx..])?;

        let ll_table = self.fse.ll.as_ref().ok_or(Error::MissingSeqTable)?;
        let of_table = self.fse.of.as_ref().ok_or(Error::MissingSeqTable)?;
        let ml_table = self.fse.ml.as_ref().ok_or(Error::MissingSeqTable)?;

        let mut ll_dec = rzstd_fse::Decoder::new(ll_table, &mut r)?;
        let mut of_dec = rzstd_fse::Decoder::new(of_table, &mut r)?;
        let mut ml_dec = rzstd_fse::Decoder::new(ml_table, &mut r)?;

        self.sequences_idx = header.n_seqs as usize;
        let dst = &mut self.sequences_buf[..self.sequences_idx];
        let mut dst_idx = 0;

        let mut ll = ll_dec.peek();
        let mut of = of_dec.peek();
        let mut ml = ml_dec.peek();

        let offset = decode_of(of, &mut r)?;
        let match_len = decode_ml(ml, &mut r)?;
        let lit_len = decode_ll(ll, &mut r)?;

        dst[dst_idx] = Sequence {
            lit_len,
            match_len,
            offset,
        };
        dst_idx += 1;

        for _ in 1..header.n_seqs {
            ll_dec.update(&mut r)?;
            ml_dec.update(&mut r)?;
            of_dec.update(&mut r)?;

            ll = ll_dec.peek();
            of = of_dec.peek();
            ml = ml_dec.peek();

            let offset = decode_of(of, &mut r)?;
            let match_len = decode_ml(ml, &mut r)?;
            let lit_len = decode_ll(ll, &mut r)?;

            dst[dst_idx] = Sequence {
                lit_len,
                match_len,
                offset,
            };
            dst_idx += 1;
        }

        tracing::debug!(
            "seqs.len={:?}; seqs={:?}",
            self.sequences_buf.len(),
            self.sequences_buf
        );

        if r.bits_remaining() > 0 {
            return Err(Error::ExtraBitsInStream(r.bits_remaining()));
        }

        self.execute_sequences()
    }
}

pub struct Header {
    n_seqs: u32,
    modes: Option<CompressionModes>,
}

impl std::fmt::Debug for Header {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SequencesHeader")
            .field("num_sequences", &self.n_seqs)
            .field("modes", &self.modes)
            .finish()
    }
}

impl Header {
    pub fn read(r: &mut impl rzstd_io::Reader) -> Result<Self, Error> {
        let first = r.read_u8()?;

        let n_seqs = match first {
            0 => {
                return Ok(Self {
                    n_seqs: 0,
                    modes: None,
                });
            }
            1..=127 => first as u32,
            128..=254 => {
                let second = r.read_u8()? as u32;
                let first = (first as u32) - 128;
                (first << 8) + second
            }
            255 => {
                let second = r.read_u8()? as u32;
                let third = r.read_u8()? as u32;

                second + (third << 8) + 0x7F00
            }
        };
        let modes = CompressionModes::read(r.read_u8()?)?;

        Ok(Self {
            n_seqs,
            modes: Some(modes),
        })
    }
}

#[derive(Debug)]
pub struct CompressionModes(u8);

impl CompressionModes {
    fn read(val: u8) -> Result<Self, Error> {
        let ret = Self(val);

        if ret.reserved() != 0 {
            return Err(Error::ReservedBitSet);
        }
        Ok(ret)
    }

    fn literal_lengths(&self) -> Mode {
        TwoBitFlag::from_u8((self.0 >> 6) & 0x3).into()
    }

    fn offsets(&self) -> Mode {
        TwoBitFlag::from_u8((self.0 >> 4) & 0x3).into()
    }

    fn match_lengths(&self) -> Mode {
        TwoBitFlag::from_u8((self.0 >> 2) & 0x3).into()
    }

    fn reserved(&self) -> u8 {
        self.0 & 0x3
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Mode {
    /// A predefined FSE distribution table is used. No distribution table will
    /// be present.
    Predefined,

    /// The table description consists of a single byte, which contains the
    /// symbol's value. This symbol will be used for all sequences.
    RLE,

    /// Standard FSE compression. A distribution table will be present. This
    /// mode must not be used when only one symbol is present;
    /// [Mode::RLE] should be used instead
    FSECompressed,

    /// The table used in the previous [TODO Block] with [n_seqs] > 0 will be
    /// used again, or if this is the first block, the table in the
    /// dictionary will be used.
    Repeat,
}

impl From<TwoBitFlag> for Mode {
    fn from(val: TwoBitFlag) -> Self {
        match val {
            TwoBitFlag::Zero => Self::Predefined,
            TwoBitFlag::One => Self::RLE,
            TwoBitFlag::Two => Self::FSECompressed,
            TwoBitFlag::Three => Self::Repeat,
        }
    }
}

fn update_table<const N: usize>(
    mode: Mode,
    dist: DefaultDistribution,
    src: &[u8],
    curr: &mut Option<rzstd_fse::DecodingTable<N>>,
) -> Result<usize, Error> {
    match mode {
        Mode::Repeat => {
            if curr.is_none() {
                return Err(Error::MissingTableForRepeat);
            }
            Ok(0)
        }
        Mode::Predefined => {
            let mut norm = rzstd_fse::NormalizedDistribution::from_predefined(
                dist.predefined_table(),
                dist.accuracy_log() as u8,
            )?;
            *curr = Some(rzstd_fse::DecodingTable::from_distribution(&mut norm)?);
            Ok(0)
        }
        Mode::RLE => {
            let sym = *src.get(0).ok_or(Error::EmptyRLESource)?;
            *curr = Some(rzstd_fse::DecodingTable::rle(sym));

            Ok(1)
        }
        Mode::FSECompressed => {
            let mut br = rzstd_io::BitReader::new(src)?;
            *curr = Some(rzstd_fse::DecodingTable::read(&mut br, dist.table_size())?);

            Ok(br.bytes_consumed())
        }
    }
}

const LL_TABLE: [(u32, u8); 36] = [
    (0, 0),
    (1, 0),
    (2, 0),
    (3, 0),
    (4, 0),
    (5, 0),
    (6, 0),
    (7, 0),
    (8, 0),
    (9, 0),
    (10, 0),
    (11, 0),
    (12, 0),
    (13, 0),
    (14, 0),
    (15, 0),
    (16, 1),
    (18, 1),
    (20, 1),
    (22, 1),
    (24, 2),
    (28, 2),
    (32, 3),
    (40, 3),
    (48, 4),
    (64, 6),
    (128, 7),
    (256, 8),
    (512, 9),
    (1024, 10),
    (2048, 11),
    (4096, 12),
    (8192, 13),
    (16384, 14),
    (32768, 15),
    (65536, 16),
];

#[inline(always)]
fn decode_ll(code: u8, r: &mut rzstd_io::ReverseBitReader) -> Result<u32, Error> {
    let &(baseline, n_bits) = &LL_TABLE[code as usize & 63];
    if n_bits == 0 {
        return Ok(baseline);
    }

    Ok(baseline + r.read(n_bits)? as u32)
}

const ML_TABLE: [(u32, u8); 53] = [
    (3, 0),
    (4, 0),
    (5, 0),
    (6, 0),
    (7, 0),
    (8, 0),
    (9, 0),
    (10, 0),
    (11, 0),
    (12, 0),
    (13, 0),
    (14, 0),
    (15, 0),
    (16, 0),
    (17, 0),
    (18, 0),
    (19, 0),
    (20, 0),
    (21, 0),
    (22, 0),
    (23, 0),
    (24, 0),
    (25, 0),
    (26, 0),
    (27, 0),
    (28, 0),
    (29, 0),
    (30, 0),
    (31, 0),
    (32, 0),
    (33, 0),
    (34, 0),
    (35, 1),
    (37, 1),
    (39, 1),
    (41, 1),
    (43, 2),
    (47, 2),
    (51, 3),
    (59, 3),
    (67, 4),
    (83, 4),
    (99, 5),
    (131, 7),
    (259, 8),
    (515, 9),
    (1027, 10),
    (2051, 11),
    (4099, 12),
    (8195, 13),
    (16387, 14),
    (32771, 15),
    (65539, 16),
];

#[inline(always)]
fn decode_ml(code: u8, r: &mut rzstd_io::ReverseBitReader) -> Result<u32, Error> {
    let &(baseline, n_bits) = &ML_TABLE[code as usize & 63];
    if n_bits == 0 {
        return Ok(baseline);
    }

    Ok(baseline + r.read(n_bits)? as u32)
}

#[inline(always)]
fn decode_of(code: u8, r: &mut rzstd_io::ReverseBitReader) -> Result<u32, Error> {
    let extra = r.read(code)?;
    Ok((1u32 << (code & 0x1F)) + extra as u32)
}
