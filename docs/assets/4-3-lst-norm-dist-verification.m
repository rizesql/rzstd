// In `rzstd_fse::decode`
pub fn read_normalized_distribution(
  src: &mut BitReader
) -> Result<...> {
  let mut remaining = 1 << accuracy_log;
  while remaining > 0 {
    if symbol_idx >= MAX_SYMBOLS {
      return Err(Error::TooManySymbols);
    }

    // ...
    let prob = /* read from `src` */;
    if prob != 0 {
      remaining -= prob;
    }
  }

  if remaining != 0 {
    return Err(
      Error::SumMismatch(remaining)
    );
  }

  Ok(/* ... */)
}
