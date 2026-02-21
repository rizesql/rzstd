use rzstd_foundation::const_assert;
use rzstd_io::{BitReader, ReverseBitReader};

use crate::Error;

const MAX_SYMBOLS: usize = 256;
const ACCURACY_LOG_RANGE: std::ops::RangeInclusive<u8> = 5..=15;

#[derive(Debug, Clone, Copy)]
#[repr(transparent)]
struct State(u16);

pub struct Decoder<'t, const N: usize> {
    state: State,
    table: &'t DecodingTable<N>,
}

impl<'t, const N: usize> Decoder<'t, N> {
    pub fn new(
        table: &'t DecodingTable<N>,
        src: &mut ReverseBitReader,
    ) -> Result<Self, Error> {
        let state = {
            let state = src.read(table.accuracy_log())?;
            State(state as u16)
        };
        tracing::debug!(
            "init FSE decoder; state={:?}; symbol={:?}",
            state.0,
            table[state]
        );

        Ok(Self { table, state })
    }

    #[inline(always)]
    pub fn peek(&self) -> u8 {
        debug_assert!((self.state.0 as usize) < self.table.entries.len());
        self.table.entries[self.state.0 as usize].symbol
    }

    #[inline(always)]
    pub fn update(&mut self, src: &mut ReverseBitReader) -> Result<(), Error> {
        debug_assert!((self.state.0 as usize) < self.table.entries.len());
        let entry = &self.table.entries[self.state.0 as usize];

        let bits = src.read(entry.n_bits)?;
        self.state = State(entry.baseline + bits as u16);
        Ok(())
    }

    #[inline(always)]
    pub fn bits_required(&self) -> u8 {
        self.table[self.state].n_bits
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NormalizedDistribution<const N: usize> {
    final_counts: [i16; MAX_SYMBOLS],
    symbol_state: [u16; MAX_SYMBOLS],
    symbol_count: usize,
    has_low_prob: bool,
    accuracy_log: u8,
}

impl<const N: usize> NormalizedDistribution<N> {
    pub fn read(src: &mut BitReader) -> Result<Self, Error> {
        assert!(N.is_power_of_two());

        let max_accuracy_log = N.trailing_zeros() as u8;
        let read = src.read(4)? as u8;
        let accuracy_log = 5 + read;

        if accuracy_log > max_accuracy_log {
            return Err(Error::AccuracyLogMismatch(max_accuracy_log, accuracy_log));
        }

        let mut final_counts = [0i16; MAX_SYMBOLS];
        let mut symbol_state = [0u16; MAX_SYMBOLS];

        let mut symbol_idx = 0;
        let mut has_low_prob = false;

        let mut remaining: i32 = 1 << accuracy_log;
        while remaining > 0 {
            if symbol_idx >= MAX_SYMBOLS {
                return Err(Error::TooManySymbols);
            }

            let max_val = remaining + 1;
            let n_bits = (32 - max_val.leading_zeros()) as u8;

            src.ensure_bits(n_bits)?;
            let val = src.peek(n_bits) as i32;
            let mask = (1 << (n_bits - 1)) - 1;
            let threshold = (1 << n_bits) - max_val - 1;
            let small = val & mask;

            let val = if small < threshold {
                src.consume(n_bits - 1);
                small
            } else if val > mask {
                src.consume(n_bits);
                val - threshold
            } else {
                src.consume(n_bits);
                val
            };

            let prob = (val - 1) as i16;

            has_low_prob |= val == 0;

            let state = if prob == -1 { 1 } else { prob };
            final_counts[symbol_idx] = prob;
            symbol_state[symbol_idx] = state as u16;
            symbol_idx += 1;

            if prob != 0 {
                remaining -= state as i32;
            } else {
                loop {
                    let skip = src.read(2)? as usize;
                    symbol_idx += skip;
                    if skip != 3 {
                        break;
                    }
                }
            }
        }

        // let mut symbol_idx = 0;
        // let mut has_low_prob = false;

        // let mut remaining: i32 = N as i32;
        // while remaining > 0 {
        //     if symbol_idx >= MAX_SYMBOLS {
        //         return Err(Error::TooManySymbols);
        //     }

        //     let n_bits = (remaining + 1).ilog2() as u8;

        //     let mut val = src.read(n_bits)? as i32;
        //     let threshold = (1 << (n_bits + 1)) - remaining - 2;

        //     if val >= threshold {
        //         let extra = src.read(1)? as i32;
        //         val += extra * ((1 << n_bits) - threshold);
        //     }

        //     let prob = (val - 1) as i16;

        //     has_low_prob |= val == 0;

        //     let state = if prob == -1 { 1 } else { prob };
        //     final_counts[symbol_idx] = prob;
        //     symbol_state[symbol_idx] = state as u16;
        //     symbol_idx += 1;

        //     if prob != 0 {
        //         remaining -= state as i32;
        //     } else {
        //         loop {
        //             if symbol_idx >= MAX_SYMBOLS {
        //                 return Err(Error::TooManySymbols);
        //             }

        //             let repeat = src.read(2)? as usize;

        //             if symbol_idx + repeat > MAX_SYMBOLS {
        //                 return Err(Error::TooManySymbols);
        //             }

        //             symbol_idx += repeat;

        //             if repeat != 3 {
        //                 break;
        //             }
        //         }
        //     }
        // }

        if remaining != 0 {
            return Err(Error::SumMismatch(remaining));
        }

        Ok(NormalizedDistribution {
            final_counts,
            symbol_state,
            symbol_count: symbol_idx,
            has_low_prob,
            accuracy_log,
        })
    }

    pub fn from_predefined(counts: &[i16], accuracy_log: u8) -> Result<Self, Error> {
        let mut final_counts = [0i16; MAX_SYMBOLS];
        let mut symbol_state = [0u16; MAX_SYMBOLS];
        let mut symbol_count = 0;
        let mut has_low_prob = false;

        for (idx, &count) in counts.iter().enumerate() {
            if idx >= MAX_SYMBOLS {
                return Err(Error::TooManySymbols);
            }

            final_counts[idx] = count;
            if count == -1 {
                has_low_prob = true;
                symbol_state[idx] = 1;
            } else {
                symbol_state[idx] = count as u16;
            }

            symbol_count = idx + 1;
        }

        Ok(Self {
            final_counts,
            symbol_state,
            symbol_count,
            has_low_prob,
            accuracy_log,
        })
    }
}

#[derive(Clone, Copy)]
#[repr(align(4))]
pub struct Entry {
    baseline: u16,
    n_bits: u8,
    symbol: u8,
}

impl std::fmt::Debug for Entry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Entry")
            .field("base_line", &self.baseline)
            .field("num_bits", &self.n_bits)
            .field("symbol", &self.symbol)
            .finish()
    }
}

const_assert!(std::mem::size_of::<Entry>() == 4);
const_assert!(std::mem::align_of::<Entry>() == 4);

#[repr(align(64))]
#[derive(Debug)]
pub struct DecodingTable<const N: usize> {
    entries: [Entry; N],
    accuracy_log: u8,
}

const_assert!(std::mem::size_of::<DecodingTable<512>>() % 64 == 0);

impl<const N: usize> DecodingTable<N> {
    pub fn read(r: &mut rzstd_io::BitReader, count: usize) -> Result<Self, Error> {
        let mut dist = NormalizedDistribution::<N>::read(r)?;
        if r.bytes_consumed() > count {
            return Err(Error::Corruption);
        }

        Self::from_distribution(&mut dist)
    }

    pub fn rle(symbol: u8) -> Self {
        let entries = [Entry {
            symbol,
            n_bits: 0,
            baseline: 0,
        }; N];
        Self {
            entries,
            accuracy_log: 0,
        }
    }

    pub fn from_distribution(
        dist: &mut NormalizedDistribution<N>,
    ) -> Result<Self, Error> {
        assert!(N.is_power_of_two());
        let accuracy_log = dist.accuracy_log;
        // let accuracy_log = N.trailing_zeros() as u8;

        if !ACCURACY_LOG_RANGE.contains(&accuracy_log) {
            return Err(Error::InvalidAccuracyLog(accuracy_log));
        }

        let mut entries = [Entry {
            symbol: 0,
            n_bits: 0,
            baseline: 0,
        }; N];

        let table = &mut entries[..(1 << accuracy_log) as usize];

        if !dist.has_low_prob {
            Self::spread_weights(dist, table)?;
        } else {
            Self::spread_symbols_low_prob(dist, table)?;
        }

        Self::finalize_table(table, &mut dist.symbol_state, accuracy_log)?;

        Ok(Self {
            entries,
            accuracy_log,
        })
    }

    fn spread_weights(
        dist: &NormalizedDistribution<N>,
        table: &mut [Entry],
    ) -> Result<(), Error> {
        let n = table.len();
        let step = (n >> 1) + (n >> 3) + 3;
        let mask = n - 1;

        let mut pos = 0;

        for (sym, &count) in dist.final_counts[..dist.symbol_count].iter().enumerate() {
            if count <= 0 {
                continue;
            }

            let entry = Entry {
                symbol: sym as u8,
                n_bits: 0xFF,
                baseline: 0,
            };

            let mut remaining = count as usize;
            while remaining >= 4 {
                table[pos] = entry;
                table[(pos + step) & mask] = entry;
                table[(pos + step * 2) & mask] = entry;
                table[(pos + step * 3) & mask] = entry;

                pos = (pos + step * 4) & mask;
                remaining -= 4;
            }

            while remaining > 0 {
                table[pos] = entry;
                pos = (pos + step) & mask;
                remaining -= 1;
            }
        }

        if pos != 0 {
            return Err(Error::FastSpreadAlignmentError(pos));
        }

        Ok(())
    }

    #[cold]
    fn spread_symbols_low_prob(
        dist: &NormalizedDistribution<N>,
        table: &mut [Entry],
    ) -> Result<(), Error> {
        let n = table.len();
        let step = (n >> 1) + (n >> 3) + 3;
        let mask = n - 1;

        let mut high_threshold = n;

        for (sym, &count) in dist.final_counts[..dist.symbol_count].iter().enumerate() {
            if count == -1 {
                high_threshold -= 1;
                table[high_threshold] = Entry {
                    symbol: sym as u8,
                    n_bits: 0xFF,
                    baseline: 0,
                };
            }
        }

        let mut pos = 0;
        for (sym, &count) in dist.final_counts[..dist.symbol_count].iter().enumerate() {
            if count <= 0 {
                continue;
            }

            for _ in 0..count {
                table[pos] = Entry {
                    symbol: sym as u8,
                    n_bits: 0xFF,
                    baseline: 0,
                };

                pos = (pos + step) & mask;

                while pos >= high_threshold {
                    pos = (pos + step) & mask;
                }
            }
        }

        if high_threshold == n && pos != 0 {
            return Err(Error::FastSpreadAlignmentError(pos));
        }

        Ok(())
    }

    fn finalize_table(
        table: &mut [Entry],
        symbol_state: &mut [u16; MAX_SYMBOLS],
        accuracy_log: u8,
    ) -> Result<(), Error> {
        let n = table.len() as u16;
        for entry in table.chunks_exact_mut(4).flatten() {
            if entry.n_bits == 0 {
                return Err(Error::TableUnderfilled);
            }

            let sym_idx = entry.symbol as usize;

            let state = symbol_state[sym_idx];
            if state == 0 {
                return Err(Error::InvalidState);
            }

            symbol_state[sym_idx] += 1;

            let n_bits = (accuracy_log + state.leading_zeros() as u8) - 15;

            entry.n_bits = n_bits;
            entry.baseline = (state << n_bits).wrapping_sub(n as u16);
        }

        Ok(())
    }

    const fn accuracy_log(&self) -> u8 {
        self.accuracy_log
    }

    #[inline(always)]
    pub fn table(&self) -> &[Entry] {
        &self.entries[..(1 << self.accuracy_log)]
    }
}

impl<const N: usize> std::ops::Index<State> for DecodingTable<N> {
    type Output = Entry;

    #[inline(always)]
    fn index(&self, index: State) -> &Self::Output {
        debug_assert!((index.0 as usize) < self.table().len());
        &self.entries[index.0 as usize]
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    #[test]
    fn test_rfc_appendix_a() {
        // RFC 8878 Appendix A: Literal Length Code
        // Accuracy Log = 6 (N=64)
        // Distribution:
        let counts: [i16; 36] = [
            4, 3, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 1, 1, 1, 2, 2, 2, 2, 2, 2, 2, 2, 2, 3,
            2, 1, 1, 1, 1, 1, -1, -1, -1, -1,
        ];

        let mut final_counts = [0i16; MAX_SYMBOLS];
        let mut symbol_state = [0u16; MAX_SYMBOLS];

        for (i, &count) in counts.iter().enumerate() {
            final_counts[i] = count;
            symbol_state[i] = if count == -1 { 1 } else { count as u16 };
        }

        let mut dist = NormalizedDistribution::<64> {
            final_counts,
            symbol_state,
            symbol_count: 36,
            has_low_prob: true,
            accuracy_log: 6,
        };

        let table = DecodingTable::<64>::from_distribution(&mut dist)
            .expect("Table construction failed");

        // Verify against Appendix A Table
        // State | Symbol | Number_Of_Bits | Base
        let expected = [
            (0, 0, 4, 0),
            (1, 0, 4, 16),
            (2, 1, 5, 32),
            (3, 3, 5, 0),
            (4, 4, 5, 0),
            (5, 6, 5, 0),
        ];

        for (state_idx, sym, nb, base) in expected {
            let entry = table.entries[state_idx];
            assert_eq!(entry.symbol, sym, "State {}: Symbol mismatch", state_idx);
            assert_eq!(entry.n_bits, nb, "State {}: Bits mismatch", state_idx);
            assert_eq!(entry.baseline, base, "State {}: Base mismatch", state_idx);
        }

        // Verify a few late states from Appendix A
        // 60 | 35 | 6 | 0
        // 63 | 32 | 6 | 0
        let entry_60 = table.entries[60];
        assert_eq!(entry_60.symbol, 35);
        assert_eq!(entry_60.n_bits, 6);
        assert_eq!(entry_60.baseline, 0);

        let entry_63 = table.entries[63];
        assert_eq!(entry_63.symbol, 32);
        assert_eq!(entry_63.n_bits, 6);
        assert_eq!(entry_63.baseline, 0);
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(1000))]

        #[test]
        fn test_fuzz_distribution_256(
            weights in proptest::collection::vec(0u32..1000, 2..200)
        ) {
            const N: usize = 256;

            let sum: u64 = weights.iter().map(|&x| x as u64).sum();
            if sum == 0 {
                return Ok(());
            }

            let mut final_counts = [0i16; MAX_SYMBOLS];
            let mut symbol_state = [0u16; MAX_SYMBOLS];
            let mut current_sum = 0;

            for (i, &w) in weights.iter().enumerate() {
                let val = ((w as u64 * N as u64) / sum) as i16;
                final_counts[i] = val;
                current_sum += val;
            }

            let diff = N as i16 - current_sum;
            if diff > 0 {
                final_counts[0] += diff;
            } else if diff < 0 {
                final_counts[0] += diff;
            }

            if final_counts[0] <= 0 {
                final_counts[0] = 1;
                let current: i16 = final_counts.iter().sum();
                if current != N as i16 {
                     final_counts[0] += N as i16 - current;
                }
            }

            for (i, &count) in final_counts.iter().enumerate() {
                if count != 0 {
                     symbol_state[i] = count as u16;
                }
            }

            if final_counts.iter().any(|&x| x < 0) {
                return Ok(());
            }

            let mut dist = NormalizedDistribution::<N> {
                final_counts,
                symbol_state,
                symbol_count: weights.len(),
                has_low_prob: false,
                accuracy_log: current_sum as u8
            };

            let _ = DecodingTable::<N>::from_distribution(&mut dist)?;
        }
    }
}
