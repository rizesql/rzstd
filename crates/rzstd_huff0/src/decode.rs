use rzstd_foundation::const_assert;

use crate::errors::Error;

const MAX_BITS: u8 = 11;
const_assert!(MAX_BITS <= 11);

const TABLE_SIZE: usize = 1 << MAX_BITS;

const FSE_ACCURACY_LOG: u8 = 6;
const FSE_TABLE_SIZE: usize = 1 << FSE_ACCURACY_LOG;
const_assert!(FSE_TABLE_SIZE == 64);

pub struct Decoder<'t, const N: usize = TABLE_SIZE> {
    table: &'t DecodingTable<N>,
    state: u64,
}

impl<'t, const N: usize> Decoder<'t, N> {
    pub fn new(table: &'t DecodingTable<N>, r: &mut rzstd_io::ReverseBitReader) -> Self {
        let state = r.read_padded(table.max_bits) as u64;
        Self { table, state }
    }

    #[inline(always)]
    pub fn decode(&mut self, r: &mut rzstd_io::ReverseBitReader) -> u8 {
        debug_assert!((self.state as usize) < self.table.entries().len());
        let state = self.table.entries[self.state as usize];
        let new_bits = r.read_padded(state.n_bits);

        self.state <<= state.n_bits;
        self.state &= self.table.entries().len() as u64 - 1;
        self.state |= new_bits;

        state.symbol
    }
}

#[repr(align(4))]
#[derive(Clone, Copy)]
pub struct Entry {
    symbol: u8,
    n_bits: u8,
}

impl std::fmt::Debug for Entry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Entry")
            .field("symbol", &self.symbol)
            .field("num_bits", &self.n_bits)
            .finish()
    }
}

#[repr(align(64))]
#[derive(Debug)]
pub struct DecodingTable<const N: usize = TABLE_SIZE> {
    entries: [Entry; N],
    n_entries: usize,
    max_bits: u8,
}
const_assert!(std::mem::size_of::<DecodingTable>() % 64 == 0);

impl<const N: usize> DecodingTable<N> {
    pub fn read(src: &[u8]) -> Result<(Self, usize), Error> {
        tracing::debug!("reading HUFF0 table");
        tracing::debug!("src.len={:?}; src={:?}", src.len(), src);

        let mut weights = [0u8; 256];
        let (weights_count, consumed) = Self::read_weights(src, &mut weights)?;
        tracing::debug!(
            "weights.len={:?}; weights={:?}",
            weights[..weights_count].len(),
            &weights[..weights_count]
        );

        for &w in &weights[..weights_count] {
            if w > MAX_BITS {
                return Err(Error::WeightTooLarge(w, MAX_BITS));
            }
        }

        let table = Self::from_weights(&weights[..weights_count])?;
        tracing::debug!(
            "huff0.len={:?}; huff0={:?}",
            table.n_entries,
            table.entries()
        );
        Ok((table, consumed))
    }

    fn from_weights(weights: &[u8]) -> Result<Self, Error> {
        let mut sum = 0u32;
        let mut max_w = 0u8;
        let mut bit_rank = [0u32; (MAX_BITS + 1) as usize];

        for &w in weights {
            if w <= 0 {
                continue;
            }

            sum += 1 << (w - 1);
            max_w = max_w.max(w);
            bit_rank[w as usize] += 1;
        }

        if sum == 0 {
            return Err(Error::ZeroWeightSum);
        }

        let max_bits = sum.ilog2() as u8 + 1;
        let target = 1 << max_bits;
        let remainder = target - sum;

        if remainder == 0 || !remainder.is_power_of_two() {
            return Err(Error::InvalidInferredWeight(remainder));
        }

        let inferred_weight = remainder.ilog2() as u8 + 1;
        bit_rank[inferred_weight as usize] += 1;

        let mut next_code = [0u32; (MAX_BITS + 1) as usize];
        let mut curr = 0u32;

        for w in 1..=max_bits as usize {
            next_code[w] = curr;
            curr += bit_rank[w] << (w - 1);
        }

        if curr != target {
            return Err(Error::TableUnderflow);
        }

        let mut entries = [Entry {
            symbol: 0,
            n_bits: 0,
        }; N];

        for (sym, &w) in weights
            .iter()
            .chain(std::iter::once(&inferred_weight))
            .enumerate()
        {
            if w <= 0 {
                continue;
            }

            let code_start = next_code[w as usize];
            let n_bits = max_bits - (w - 1);
            let num_slots = 1 << (w - 1);

            for i in 0..num_slots {
                let idx = (code_start as usize) + i;
                entries[idx] = Entry {
                    symbol: sym as u8,
                    n_bits,
                };
            }

            next_code[w as usize] += num_slots as u32;
        }

        Ok(Self {
            entries,
            n_entries: target as usize,
            max_bits,
        })
    }

    fn read_weights(src: &[u8], out: &mut [u8; 256]) -> Result<(usize, usize), Error> {
        let header = *src
            .first()
            .ok_or(Error::IO(rzstd_io::Error::NotEnoughBits {
                requested: 8,
                remaining: 0,
            }))?;
        let src = &src[1..];

        if header >= 128 {
            let count = header - 127;
            let consumed = Self::read_weights_direct(src, out, count)?;
            Ok((count as usize, consumed + 1))
        } else {
            let num_weights = Self::read_weights_compressed(src, out, header)?;
            Ok((num_weights, header as usize + 1))
        }
    }

    fn read_weights_direct(
        src: &[u8],
        out: &mut [u8; 256],
        count: u8,
    ) -> Result<usize, Error> {
        assert!(count <= 128);
        let count = count as usize;

        let mut r = rzstd_io::BitReader::new(src)?;

        let mut idx = 0usize;
        let mut remaining_bytes = (count + 1) / 2;

        while remaining_bytes >= 7 && idx + 14 <= count {
            assert!(idx + count <= out.len());

            let chunk = r.read(56)?;
            let dst = &mut out[idx..][..14];

            for i in 0..7 {
                let byte = (chunk >> (i * 8)) as u8;
                dst[2 * i] = byte >> 4;
                dst[2 * i + 1] = byte & 0xF;
            }

            idx += 14;
            remaining_bytes -= 7;
        }

        if remaining_bytes == 0 {
            return Ok(r.bytes_consumed());
        }

        let chunk = r.read((remaining_bytes * 8) as u8)?;

        for i in 0..remaining_bytes {
            let byte = (chunk >> (i * 8)) as u8;
            if idx < count {
                out[idx] = byte >> 4;
                idx += 1;
            }

            if idx < count {
                out[idx] = byte & 0xF;
                idx += 1;
            }
        }

        Ok(r.bytes_consumed())
    }

    fn read_weights_compressed(
        src: &[u8],
        out: &mut [u8; 256],
        compressed_size: u8,
    ) -> Result<usize, Error> {
        let compressed_size = compressed_size as usize;
        if src.len() < compressed_size {
            return Err(Error::IO(rzstd_io::Error::NotEnoughBits {
                requested: compressed_size * 8,
                remaining: src.len() * 8,
            }));
        }

        let mut table_reader = rzstd_io::BitReader::new(src)?;
        let table = rzstd_fse::DecodingTable::<FSE_TABLE_SIZE>::read(
            &mut table_reader,
            compressed_size,
        )?;
        let mut br = rzstd_io::ReverseBitReader::new(
            &src[table_reader.bytes_consumed()..compressed_size],
        )?;

        let mut dec1 = rzstd_fse::Decoder::new(&table, &mut br)?;
        let mut dec2 = rzstd_fse::Decoder::new(&table, &mut br)?;

        let mut idx = 0;
        while idx < out.len() {
            out[idx] = dec1.peek();
            idx += 1;

            if dec1.bits_required() as usize > br.bits_remaining() {
                out[idx] = dec2.peek();
                idx += 1;
                break;
            }

            dec1.update(&mut br)?;

            out[idx] = dec2.peek();
            idx += 1;

            if dec2.bits_required() as usize > br.bits_remaining() {
                out[idx] = dec1.peek();
                idx += 1;
                break;
            }

            dec2.update(&mut br)?;
        }

        Ok(idx)
    }

    pub fn entries(&self) -> &[Entry] {
        &self.entries[..self.n_entries]
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    #[test]
    fn test_rfc_example_decoding() -> Result<(), Error> {
        let weights = [4, 3, 2, 0, 1];
        let table = DecodingTable::<64>::from_weights(&weights)?;

        let data = [0x80, 0x0D];
        let mut reader = rzstd_io::ReverseBitReader::new(&data)?;
        let mut decoder = Decoder::new(&table, &mut reader);

        let sym = decoder.decode(&mut reader);
        assert_eq!(sym, 0, "Expected A (0)");

        let sym = decoder.decode(&mut reader);
        assert_eq!(sym, 1, "Expected B (1)");

        let sym = decoder.decode(&mut reader);
        assert_eq!(sym, 4, "Expected E (4)");

        let sym = decoder.decode(&mut reader);
        assert_eq!(sym, 5, "Expected F (5)");

        assert_eq!(reader.bits_remaining(), 0);
        Ok(())
    }

    #[test]
    fn test_simple_inferred_weight() {
        let weights = [1u8];
        let table = DecodingTable::<256>::from_weights(&weights).expect("Should succeed");

        assert_eq!(table.max_bits, 1);
        assert_eq!(table.entries[0].symbol, 0);
        assert_eq!(table.entries[1].symbol, 1);
    }

    #[test]
    fn test_invalid_weight_sum() {
        let weights = [3, 2];
        assert!(DecodingTable::<256>::from_weights(&weights).is_ok());

        let weights_bad = [3, 3, 2];
        assert!(DecodingTable::<256>::from_weights(&weights_bad).is_err());
    }

    #[test]
    fn test_rfc_example() {
        let weights = [4, 3, 2, 0, 1];

        let table = DecodingTable::<64>::from_weights(&weights)
            .expect("RFC example should be valid");

        assert_eq!(table.max_bits, 4);
    }

    #[test]
    fn test_read_direct() {
        let data = [129, 0x43];
        let (table, _) = DecodingTable::<64>::read(&data).expect("Read direct failed");

        assert_eq!(table.max_bits, 4);
    }

    #[test]
    fn test_inferred_weight_boundaries() {
        let w1 = [1, 1, 1];
        let t1 = DecodingTable::<2048>::from_weights(&w1).unwrap();
        assert_eq!(t1.max_bits, 2);

        let w_max = [11, 11];
        let t_max = DecodingTable::<2048>::from_weights(&w_max);
        assert!(t_max.is_err(), "Should fail: no room for inferred weight");
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(500))]

        #[test]
        fn test_fuzz_from_weights(
            weights in proptest::collection::vec(0u8..=MAX_BITS, 2..100)
        ) {
            let _ = DecodingTable::<2048>::from_weights(&weights);
        }

        #[test]
        fn test_fuzz_read_direct(
            count in 1u8..128,
            payload in proptest::collection::vec(any::<u8>(), 0..100)
        ) {
             let header = 127 + count;
             let mut buf = vec![header];
             let needed = (count as usize + 1) / 2;

             let mut payload = payload;
             if payload.len() > needed {
                 payload.truncate(needed);
             }
             buf.extend(payload);

             let _ = DecodingTable::<2048>::read(&buf);
        }

        #[test]
        fn test_table_consistency(
            weights in prop::collection::vec(0..=11u8, 1..255)
        ) {
            let mut sum = 0u32;
            for &w in &weights {
                if w > 0 { sum += 1 << (w - 1); }
            }

            if sum > 0 && sum < (1 << 11) {
                let target = sum.next_power_of_two();
                let remainder = target - sum;

                if remainder.is_power_of_two() {
                    if let Ok(table) = DecodingTable::<2048>::from_weights(&weights) {
                        assert!(table.max_bits <= 11);

                        let table_size = 1 << table.max_bits;
                        for i in 0..table_size {
                            assert!(table.entries[i].n_bits > 0, "Empty slot at index {}", i);
                        }
                    }
                }
            }
        }

        #[test]
        fn test_decoder_no_panic_on_random_bits(
            weights in prop::collection::vec(0u8..=11, 2..20),
            random_data in prop::collection::vec(any::<u8>(), 1..64)
        ) {
            if let Ok(table) = DecodingTable::<2048>::from_weights(&weights) {
                if random_data.is_empty() || random_data[random_data.len()-1] == 0 { return Ok(()); }

                let mut reader = rzstd_io::ReverseBitReader::new(&random_data)?;
                let mut decoder = Decoder::new(&table, &mut reader);

                for _ in 0..20 {
                    if reader.bits_remaining() < table.max_bits as usize { break; }
                    let _ = decoder.decode(&mut reader);
                }
            }
        }
    }
}
