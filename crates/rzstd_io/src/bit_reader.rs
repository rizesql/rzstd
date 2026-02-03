use crate::Error;

#[derive(Debug)]
pub struct BitReader<'src> {
    src: &'src [u8],
    buf: u64,
    bit_count: u8,
    index: usize,
}

impl<'src> BitReader<'src> {
    pub fn new(src: &'src [u8]) -> Result<Self, Error> {
        if src.is_empty() {
            return Err(Error::EmptyStream);
        }

        let mut ret = Self {
            src,
            buf: 0,
            bit_count: 0,
            index: 0,
        };
        ret.refill();

        Ok(ret)
    }

    #[inline(always)]
    pub fn read(&mut self, n_bits: u8) -> Result<u64, Error> {
        assert!(n_bits <= 56);

        if self.bit_count < n_bits {
            self.refill();

            if self.bit_count < n_bits {
                return Err(Error::NotEnoughBits {
                    requested: n_bits as usize,
                    remaining: self.bit_count as usize + self.src.len() * 8,
                });
            }
        }

        let ret = self.peek(n_bits);
        self.buf >>= n_bits;
        self.bit_count -= n_bits;
        Ok(ret)
    }

    #[inline(always)]
    pub fn peek(&self, n_bits: u8) -> u64 {
        assert!(n_bits <= self.bit_count);

        self.buf & ((1u64 << n_bits) - 1)
    }

    #[inline(always)]
    pub fn bytes_consumed(&self) -> usize {
        self.index - (self.bit_count as usize / 8)
    }

    #[cold]
    fn refill(&mut self) {
        debug_assert!(self.bit_count < 64);

        let count = ((64 - self.bit_count) / 8) as usize;
        if count == 0 {
            return;
        }

        let to_read = count.min(self.src.len());
        if to_read < 8 {
            return self.refill_cold(to_read);
        }

        assert_eq!(self.bit_count, 0);

        let buf = {
            let bytes = self.src[..8]
                .try_into()
                .expect("slice length is guaranteed to be 8");
            u64::from_le_bytes(bytes)
        };

        self.buf = buf;
        self.bit_count = 64;
        self.src = &self.src[8..];
        self.index += 8;
    }

    #[cold]
    fn refill_cold(&mut self, count: usize) {
        let to_read = count.min(self.src.len());

        for (idx, &byte) in self.src[..to_read].iter().enumerate() {
            self.buf |= (byte as u64) << (self.bit_count + (idx as u8) * 8);
        }

        self.bit_count += (to_read * 8) as u8;
        self.src = &self.src[to_read..];
        self.index += to_read;
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;
    use crate::tests::*;

    #[test]
    fn test_bit_order() -> Result<(), Error> {
        let mut br = BitReader::new(&[0x1D])?;

        assert_eq!(br.read(1)?, 1);
        assert_eq!(br.read(1)?, 0);
        assert_eq!(br.read(1)?, 1);
        assert_eq!(br.read(1)?, 1);
        assert_eq!(br.read(1)?, 1);
        assert_eq!(br.read(1)?, 0);

        Ok(())
    }

    #[test]
    fn test_refill_cold_byte_order() -> Result<(), Error> {
        let mut br = BitReader::new(&[0xAA, 0xBB])?;

        assert_eq!(br.read(8)?, 0xAA);
        assert_eq!(br.read(8)?, 0xBB);

        Ok(())
    }

    #[test]
    fn test_refill_hot_path() -> Result<(), Error> {
        let mut br =
            BitReader::new(&[0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99])?;

        assert_eq!(br.read(8)?, 0x11);
        assert_eq!(br.read(8)?, 0x22);
        assert_eq!(br.read(8)?, 0x33);
        assert_eq!(br.read(8)?, 0x44);
        assert_eq!(br.read(8)?, 0x55);
        assert_eq!(br.read(8)?, 0x66);
        assert_eq!(br.read(8)?, 0x77);
        assert_eq!(br.read(8)?, 0x88);
        assert_eq!(br.read(8)?, 0x99);

        Ok(())
    }

    #[test]
    fn test_constructor_edge_cases() {
        assert!(matches!(BitReader::new(&[]), Err(Error::EmptyStream)));

        assert!(BitReader::new(&[0]).is_ok());
    }

    #[test]
    fn test_bytes_consumed() -> Result<(), Error> {
        let data = [0xAA, 0xBB, 0xCC, 0xDD];
        let mut br = BitReader::new(&data)?;

        // Initial: 4 bytes loaded, 32 bits buf. Consumed = 4 - 4 = 0.
        assert_eq!(br.bytes_consumed(), 0);

        br.read(4)?; // 28 bits left (3 bytes + 4 bits) -> 3 bytes full. Consumed = 4 - 3 = 1.
        assert_eq!(br.bytes_consumed(), 1);

        br.read(4)?; // 24 bits left -> 3 bytes full. Consumed = 4 - 3 = 1.
        assert_eq!(br.bytes_consumed(), 1);

        br.read(1)?; // 23 bits left -> 2 bytes full. Consumed = 4 - 2 = 2.
        assert_eq!(br.bytes_consumed(), 2);

        br.read(23)?; // 0 bits left -> 0 bytes full. Consumed = 4 - 0 = 4.
        assert_eq!(br.bytes_consumed(), 4);

        Ok(())
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(1000))]

        #[test]
        fn test_fuzz_random_reads(
            bits in proptest::collection::vec(any::<bool>(), 0..2000),
            reads in proptest::collection::vec(1u8..=56, 0..1000)
        ) {
            let src = encode_bits(&bits);
            // If bits are empty, creating reader fails
            if src.is_empty() {
                prop_assert!(matches!(BitReader::new(&src), Err(Error::EmptyStream)));
                return Ok(());
            }

            let mut br = BitReader::new(&src)?;
            let mut remaining = bits.as_slice();

            for n in reads {
                if remaining.len() < n as usize {
                     break;
                }

                let (chunk, rest) = remaining.split_at(n as usize);
                let expected = pack_bits(chunk);

                let actual = br.read(n as u8)?;
                prop_assert_eq!(actual, expected, "Mismatch reading {} bits", n);

                remaining = rest;
            }

        }
    }

    fn encode_bits(bits: &[bool]) -> Vec<u8> {
        bits.chunks(8)
            .map(|chunk| {
                chunk
                    .iter()
                    .enumerate()
                    .fold(0u8, |acc, (i, &b)| acc | ((b as u8) << i))
            })
            .collect()
    }
}
