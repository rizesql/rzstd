pub trait Reader: std::io::Read + ReadU8 + ReadU32 {}

impl<T: std::io::Read> Reader for T {}

pub trait ReadU8: std::io::Read {
    fn read_u8(&mut self) -> std::io::Result<u8>;
}

impl<T: std::io::Read> ReadU8 for T {
    #[inline]
    fn read_u8(&mut self) -> std::io::Result<u8> {
        let mut buf = [0; 1];
        self.read_exact(&mut buf)?;
        Ok(buf[0])
    }
}

pub trait ReadU32: std::io::Read {
    fn read_u32(&mut self) -> std::io::Result<u32>;
}

impl<T: std::io::Read> ReadU32 for T {
    #[inline]
    fn read_u32(&mut self) -> std::io::Result<u32> {
        let mut buf = [0; 4];
        self.read_exact(&mut buf)?;
        Ok(u32::from_le_bytes(buf))
    }
}
