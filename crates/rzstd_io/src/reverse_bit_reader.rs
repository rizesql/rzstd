use crate::Error;

#[derive(Debug)]
pub struct ReverseBitReader<'src> {
    src: &'src [u8],
    buf: u64,
    bit_count: u8,
}

impl<'src> ReverseBitReader<'src> {
    pub fn new(src: &'src [u8]) -> Result<Self, Error> {
        if src.is_empty() {
            return Err(Error::EmptyStream);
        }

        let last = src[src.len() - 1];
        if last == 0 {
            return Err(Error::MissingSentinel);
        }

        let src = &src[..src.len() - 1];
        let bit_count = (u8::BITS - last.leading_zeros() - 1) as u8;

        let buf = {
            let mask = (1 << bit_count) - 1;
            (last as u64) & mask
        };

        Ok(Self {
            src,
            buf,
            bit_count,
        })
    }

    #[inline(always)]
    pub fn bit_count(&self) -> u8 {
        self.bit_count
    }

    #[inline(always)]
    pub fn ensure_bits(&mut self, n_bits: u8) -> Result<(), Error> {
        if self.bit_count < n_bits {
            self.refill();
            if self.bit_count < n_bits {
                return Err(Error::NotEnoughBits);
            }
        }
        Ok(())
    }

    #[inline(always)]
    pub fn read(&mut self, n_bits: u8) -> Result<u64, Error> {
        assert!(n_bits <= 56);

        self.ensure_bits(n_bits)?;
        let ret = self.peek(n_bits);
        self.consume_unchecked(n_bits);

        Ok(ret)
    }

    #[inline(always)]
    pub fn read_padded(&mut self, n_bits: u8) -> u64 {
        assert!(n_bits <= 56);

        if self.bit_count < n_bits {
            self.refill();
        }

        let to_read = self.bit_count.min(n_bits);
        let ret = self.peek(to_read);
        self.consume_unchecked(to_read);

        ret
    }

    #[inline(always)]
    pub fn bits_remaining(&self) -> usize {
        self.bit_count as usize + self.src.len() * 8
    }

    #[inline(always)]
    pub fn peek(&self, n_bits: u8) -> u64 {
        assert!(n_bits <= self.bit_count);

        self.buf & ((1u64 << n_bits) - 1)
    }

    #[inline(always)]
    pub fn consume(&mut self, n_bits: u8) {
        assert!(n_bits <= self.bit_count);
        self.consume_unchecked(n_bits)
    }

    #[inline(always)]
    fn consume_unchecked(&mut self, n_bits: u8) {
        self.buf >>= n_bits;
        self.bit_count -= n_bits;
    }

    #[cold]
    fn refill(&mut self) {
        assert!(self.bit_count < 64);

        let count = ((64 - self.bit_count) / 8) as usize;
        if count == 0 {
            return;
        }

        let to_read = count.min(self.src.len());
        if to_read < 8 {
            return self.refill_cold(to_read);
        }

        assert_eq!(self.bit_count, 0);

        let start = self.src.len() - 8;
        let buf = {
            let bytes = self.src[start..start + 8]
                .try_into()
                .expect("slice length is guaranteed to be 8");
            u64::from_be_bytes(bytes)
        };

        self.buf = buf;
        self.bit_count = 64;
        self.src = &self.src[..start];
    }

    #[cold]
    fn refill_cold(&mut self, count: usize) {
        let avail = self.src.len();
        let to_read = count.min(avail);

        let start = avail - to_read;
        for (idx, &byte) in self.src[start..].iter().rev().enumerate() {
            self.buf |= (byte as u64) << (self.bit_count + (idx as u8) * 8);
        }

        self.bit_count += (to_read * 8) as u8;
        self.src = &self.src[..start];
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;
    use crate::tests::*;

    #[test]
    fn test_sentinel_and_bit_order() -> Result<(), Error> {
        let data = [0x1D];

        let mut br = ReverseBitReader::new(&data)?;

        assert_eq!(br.read(1)?, 1, "Bit 0 should be 1");
        assert_eq!(br.read(1)?, 0, "Bit 1 should be 0");
        assert_eq!(br.read(1)?, 1, "Bit 2 should be 1");
        assert_eq!(br.read(1)?, 1, "Bit 3 should be 1");

        assert!(matches!(br.read(1), Err(Error::NotEnoughBits)));

        Ok(())
    }

    #[test]
    fn test_refill_cold_byte_order() -> Result<(), Error> {
        let data = [0xAA, 0xBB, 0x01];
        let mut br = ReverseBitReader::new(&data)?;

        assert_eq!(br.read(8)?, 0xBB);
        assert_eq!(br.read(8)?, 0xAA);

        Ok(())
    }

    #[test]
    fn test_refill_hot_path() -> Result<(), Error> {
        let data = [0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x01];
        let mut br = ReverseBitReader::new(&data)?;

        assert_eq!(br.read(8)?, 0x88);
        assert_eq!(br.read(8)?, 0x77);

        Ok(())
    }

    #[test]
    fn test_stream_consumption() -> Result<(), Error> {
        let data = [0b0000_1010];
        let mut br = ReverseBitReader::new(&data)?;

        assert_eq!(br.read(1)?, 0);
        assert_eq!(br.read(1)?, 1);
        assert_eq!(br.read(1)?, 0);

        assert!(matches!(br.read(1), Err(Error::NotEnoughBits)));
        Ok(())
    }

    #[test]
    fn test_constructor_edge_cases() -> Result<(), Error> {
        assert!(matches!(
            ReverseBitReader::new(&[]).err(),
            Some(Error::EmptyStream)
        ));

        assert!(matches!(
            ReverseBitReader::new(&[0]).err(),
            Some(Error::MissingSentinel)
        ));

        let mut br = ReverseBitReader::new(&[0x01])?;
        assert!(matches!(br.read(1).err(), Some(Error::NotEnoughBits)));

        Ok(())
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(10000))]

        #[test]
        fn test_fuzz_random_reads(
          bits in proptest::collection::vec(any::<bool>(), 0..2000),
          reads in proptest::collection::vec(1u8..=56, 0..1000)
        ) {
            let src = encode_bits(&bits);
            let mut br = ReverseBitReader::new(&src)?;

            let mut remaining = bits.as_slice();

            for n in reads {
              if remaining.len() < n as usize {
                break;
              }

              let (chunk, rest) = remaining.split_at(n as usize);

              let expected = pack_bits(chunk);
              let actual = br.read(n as u8)?;

              prop_assert_eq!(actual, expected,
                  "Mismatch reading {} bits ({} bits remaining)", n, remaining.len());

              remaining = rest;
            }

            if remaining.len() < 56 {
                let too_many = (remaining.len() + 1) as u8;
                prop_assert!(br.read(too_many).is_err());
            }
        }
    }

    fn encode_bits(bits: &[bool]) -> Vec<u8> {
        let rem = bits.len() % 8;
        let (head, tail) = bits.split_at(rem);

        let head = pack_bits(head) as u8 | (1 << rem);

        tail.rchunks(8)
            .map(|chunk| pack_bits(chunk) as u8)
            .chain(std::iter::once(head))
            .collect()
    }
}
