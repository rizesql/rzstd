// In `rzstd_decompress::block`

impl Header {
  pub fn decompressed_size(&self) -> Option<u32> {
    match self.block_type {
      Type::Raw | Type::RLE => Some(self.block_size),
      Type::Compressed => None,
    }
  }
}

// In Context::block execution loop:
match header.block_type {
  Type::Raw | Type::RLE => {
    // The type system requires unpacking
    // the `Option`, preventing unverified
    // downstream operations
    let count = header.decompressed_size()
      .ok_or(Error::MissingBlockSize)?;
    self.window_buf
      .read_from(&mut self.src, count)?;
  },
  Type::Compressed => {
    // entropy decoding and signal reconstruction
  }
}
