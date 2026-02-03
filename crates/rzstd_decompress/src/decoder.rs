use crate::{MAGIC_NUM, context::Context, errors::Error, frame};

pub struct Decoder<'b, R: rzstd_io::Reader> {
    ctx: Context<'b, R>,
}

const CHUNK: usize = 64 * 1024;

impl<'b, R: rzstd_io::Reader> Decoder<'b, R> {
    pub fn new(src: R, dst: &'b mut [u8], window_size: usize) -> Self {
        Decoder {
            ctx: Context::new(src, dst, window_size),
        }
    }

    pub fn decode(&mut self, mut writer: impl std::io::Write) -> Result<(), Error> {
        while self.decode_frame(&mut writer)? {}
        Ok(())
    }

    fn decode_frame(&mut self, writer: &mut impl std::io::Write) -> Result<bool, Error> {
        let magic_num = match self.ctx.src.read_u32() {
            Ok(it) => it,
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(false),
            Err(e) => return Err(Error::from(e)),
        };
        if magic_num != MAGIC_NUM {
            return Err(Error::InvalidMagicNum(magic_num));
        }

        println!("DECODING FRAME");
        let frame = frame::Header::read(&mut self.ctx.src)?;
        let window_size = frame.window_size()? as usize;

        self.ctx.reset(window_size);
        // println!("{:?}", &self.ctx);

        let mut flushed_idx = 0;

        loop {
            println!("  DECODING BLOCK");
            let last = self.ctx.block()?;
            let current_idx = self.ctx.window_buf.index();

            if current_idx < flushed_idx {
                flushed_idx = 0;
            }

            let available = current_idx.saturating_sub(flushed_idx);
            if available >= CHUNK || last {
                let data = &self.ctx.window_buf.as_slice()[flushed_idx..current_idx];
                writer.write_all(data).map_err(Error::from)?;
                flushed_idx = current_idx;
            }

            if last {
                break;
            }
        }

        if frame.has_checksum() {
            let checksum = self.ctx.src.read_u32()?;
            // TODO: checksum
        }

        Ok(true)
    }
}
