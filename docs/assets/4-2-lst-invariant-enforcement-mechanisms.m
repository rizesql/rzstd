// In `rzstd_decompress::frame::Header`
pub fn read(
  src: &mut impl rzstd_io::Reader
) -> Result<Self, Error> {
  let descriptor = HeaderDescriptor::read(
    src.read_u8()?
  )?;

  // Conditionally parse sizes based on
  // validated descriptor flags
  let content_size = match descriptor.fcs_field_size() {
    FCSFieldSize::Zero => None,
    size => {
      // read `size` bytes from `src`
    }
  };

  let header = Self {
    descriptor,
    content_size,
    /* ... */
  };

  // Final semantic invariant check
  // before yielding the type
  if header.descriptor.is_single_segment() && header.content_size.is_none() {
    return Err(
      Error::MissingFrameContentSize
    );
  }

  Ok(header)
}
