// Inside rzstd_decompress::window::Window

pub fn copy_within(
  &mut self, offset: usize, length: usize
) -> Result<(), Error> {
  // pre-validation
  let available = self.index.min(self.size);
  if offset == 0 || offset > available {
    return Err(Error::CopyOutOfBounds);
  }

  let start = self.index - offset;

  // signal regeneration dispatch
  if offset >= length {
    // disjoint segments; direct copy
    self.buf.copy_within(
      start..start + length,
      self.index
    );
  } else if offset == 1 {
    // constant signal; direct memory fill
    let val = self.buf[start];
    self.buf[self.index..self.index + length].fill(val);
  } else {
    // periodic signal; exponential copy
    let initial_copy = std::cmp::min(offset, length);
    self.buf.copy_within(
      start..start + initial_copy,
      self.index
    );

    let mut copied = initial_copy;
    while copied < length {
      let copy_len = std::cmp::min(copied, length - copied);
      self.buf.copy_within(
        self.index..self.index + copy_len,
        self.index + copied
      );
      copied += copy_len;
    }
  }

  self.index += length;
  Ok(())
}
