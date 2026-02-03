use crate::{MAX_BLOCK_SIZE, prelude::*};

#[derive(Debug)]
pub struct Window<'b> {
    buf: &'b mut [u8],
    size: usize,
    index: usize,
}

impl<'b> Window<'b> {
    pub fn new(buf: &'b mut [u8], size: usize) -> Self {
        Self {
            buf,
            size,
            index: 0,
        }
    }

    #[inline(always)]
    pub fn index(&self) -> usize {
        self.index
    }

    pub fn reset(&mut self, size: usize) {
        assert!(self.buf.len() >= size + MAX_BLOCK_SIZE as usize);

        self.size = size;
        self.index = 0;
    }

    #[inline(always)]
    fn shift(&mut self) {
        if self.index <= self.size {
            return;
        }

        self.buf.copy_within(self.index - self.size..self.index, 0);
        self.index = self.size;
    }

    #[inline(always)]
    pub fn as_slice(&self) -> &[u8] {
        &self.buf[..self.index]
    }

    #[inline(always)]
    pub fn read_from(
        &mut self,
        src: &mut impl rzstd_io::Reader,
        len: usize,
    ) -> Result<(), Error> {
        if self.index + len > self.buf.len() {
            self.shift();
        }

        let target = &mut self.buf[self.index..self.index + len];
        src.read_exact(target)?;
        self.index += len;
        Ok(())
    }

    #[inline(always)]
    pub fn push_buf(&mut self, data: &[u8]) {
        if self.index + data.len() > self.buf.len() {
            self.shift();
        }

        self.buf[self.index..self.index + data.len()].copy_from_slice(data);
        self.index += data.len();
    }

    #[inline(always)]
    pub fn push_rle(&mut self, byte: u8, count: usize) {
        if self.index + count > self.buf.len() {
            self.shift();
        }

        self.buf[self.index..self.index + count].fill(byte);
        self.index += count
    }

    #[inline(always)]
    pub fn copy_within(&mut self, offset: usize, n_bytes: usize) -> Result<(), Error> {
        debug_assert!(n_bytes <= MAX_BLOCK_SIZE as usize);

        let available = self.index.min(self.size);
        if offset == 0 || offset > available {
            return Err(Error::CopiedSizeOutOfBounds);
        }

        if self.index + n_bytes > self.buf.len() {
            self.shift();
        }

        let start = self.index - offset;
        if offset >= n_bytes {
            self.buf.copy_within(start..start + n_bytes, self.index);
        } else {
            for idx in 0..n_bytes {
                self.buf[self.index + idx] = self.buf[start + idx];
            }
        }

        self.index += n_bytes;
        Ok(())
    }
}
