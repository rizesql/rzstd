use crate::{context::Context, prelude::*};

impl<R: rzstd_io::Reader> Context<'_, R> {
    pub fn execute_sequences(&mut self) -> Result<(), Error> {
        tracing::debug!("\nsequence execution \n");

        let literals = &self.literals_buf[..self.literals_idx];
        let sequences = &self.sequences_buf[..self.sequences_idx];
        let offset_hist = &mut self.offset_hist;

        let mut lit_idx = 0usize;
        let mut literal: &[u8];

        for seq in sequences {
            let lit_len = seq.lit_len as usize;
            if lit_len > 0 {
                let next_lit_idx = lit_idx.checked_add(lit_len).ok_or(
                    Error::LiteralsBufferOverread {
                        idx: lit_idx,
                        len: lit_len,
                    },
                )?;
                if next_lit_idx > literals.len() {
                    return Err(Error::LiteralsBufferOverread {
                        idx: lit_idx,
                        len: lit_len,
                    });
                }

                literal = &literals[lit_idx..next_lit_idx];
                self.window_buf.push_buf(literal);
                lit_idx += lit_len;
            } else {
                literal = &[];
            }

            let offset = update_offset_hist(offset_hist, seq.offset, lit_len)?;

            let match_len = seq.match_len as usize;

            tracing::debug!("offset_hist={:?}", offset_hist);
            tracing::debug!(
                "lit={:?}; offset={}, match={:?}",
                literal,
                offset,
                match_len
            );

            if match_len > 0 {
                self.window_buf.copy_within(offset, match_len)?;
            }
        }

        if lit_idx < literals.len() {
            self.window_buf.push_buf(&literals[lit_idx..]);
        }
        tracing::debug!(
            "lit_remainder.len={:?}, lit_remainder={:?}",
            literals[lit_idx..].len(),
            &literals[lit_idx..]
        );
        self.literals_idx = 0;
        Ok(())
    }
}

fn update_offset_hist(
    history: &mut [usize; 3],
    offset: u32,
    lit_len: usize,
) -> Result<usize, Error> {
    let next_offset = if lit_len > 0 {
        match offset {
            1..=3 => history[offset as usize - 1],
            _ => {
                //new offset
                offset as usize - 3
            }
        }
    } else {
        match offset {
            1..=2 => history[offset as usize],
            3 => history[0] - 1,
            _ => {
                //new offset
                offset as usize - 3
            }
        }
    };

    //update history
    if lit_len > 0 {
        match offset {
            1 => {
                //nothing
            }
            2 => {
                history[1] = history[0];
                history[0] = next_offset;
            }
            _ => {
                history[2] = history[1];
                history[1] = history[0];
                history[0] = next_offset;
            }
        }
    } else {
        match offset {
            1 => {
                history[1] = history[0];
                history[0] = next_offset;
            }
            2 => {
                history[2] = history[1];
                history[1] = history[0];
                history[0] = next_offset;
            }
            _ => {
                history[2] = history[1];
                history[1] = history[0];
                history[0] = next_offset;
            }
        }
    }

    Ok(next_offset)
}
