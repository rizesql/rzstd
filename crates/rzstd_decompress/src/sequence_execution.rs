use crate::{context::Context, prelude::*};

impl<R: rzstd_io::Reader> Context<'_, R> {
    pub fn execute_sequences(&mut self, block_size: u32) -> Result<(), Error> {
        self.sequence_section(block_size)?;

        let mut lit_idx = self.literals_idx;
        let literals = &self.literals_buf[..];

        for seq in &self.sequences_buf {
            let lit_len = seq.lit_len as usize;

            let next_lit_idx =
                lit_idx
                    .checked_add(lit_len)
                    .ok_or(Error::LiteralsBufferOverread {
                        idx: lit_idx,
                        len: lit_len,
                    })?;
            if next_lit_idx > literals.len() {
                return Err(Error::LiteralsBufferOverread {
                    idx: lit_idx,
                    len: lit_len,
                });
            }

            let lit_chunk = &literals[lit_idx..next_lit_idx];
            self.window_buf.push_buf(lit_chunk);
            lit_idx = next_lit_idx;

            let match_len = seq.match_len as usize;
            let offset = update_offset_hist(&mut self.offset_hist, seq.offset, lit_len)?;

            self.window_buf.copy_within(offset, match_len)?;
        }

        if lit_idx < literals.len() {
            self.window_buf.push_buf(&literals[lit_idx..]);
        }
        self.literals_idx = literals.len();
        Ok(())
    }
}

fn update_offset_hist(
    history: &mut [usize; 3],
    of_raw: u32,
    lit_len: usize,
) -> Result<usize, Error> {
    let offset = match of_raw {
        1 => history[0],
        2 => {
            let of = history[1];
            history[1] = history[0];
            history[0] = of;
            of
        }
        3 => {
            let of = if lit_len == 0 {
                history[0]
                    .checked_sub(1)
                    .ok_or(Error::InvalidOffsetCode(history[0] as u32))?
            } else {
                history[2]
            };

            history[2] = history[1];
            history[1] = history[0];
            history[0] = of;
            of
        }
        _ => {
            let of = (of_raw as usize)
                .checked_sub(3)
                .ok_or(Error::InvalidOffsetCode(of_raw))?;

            history[2] = history[1];
            history[1] = history[0];
            history[0] = of;
            of
        }
    };

    if offset == 0 {
        return Err(Error::ZeroOffset);
    }

    Ok(offset)
}
