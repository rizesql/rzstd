use crate::{
    LL_DIST, MAX_BLOCK_SIZE, ML_DIST, OF_DIST, sequences_section::Sequence,
    window::Window,
};

pub struct Context<'out, R: rzstd_io::Reader> {
    pub src: R,
    pub window_buf: Window<'out>,

    pub literals_buf: Vec<u8>,
    pub literals_idx: usize,

    pub sequences_buf: Vec<Sequence>,

    pub huff: HuffContext,
    pub fse: FSEContext,
    pub offset_hist: [usize; 3],

    pub scratch_buf: Vec<u8>,
}

impl<'out, R: rzstd_io::Reader> Context<'out, R> {
    pub fn new(src: R, dst: &'out mut [u8], window_size: usize) -> Self {
        Self {
            src,
            window_buf: Window::new(dst, window_size),
            literals_buf: vec![0; MAX_BLOCK_SIZE as usize],
            sequences_buf: vec![Sequence::default(); MAX_BLOCK_SIZE as usize],
            // literals_buf: Vec::with_capacity(MAX_BLOCK_SIZE as usize),
            // sequences_buf: Vec::with_capacity(MAX_BLOCK_SIZE as usize),
            literals_idx: 0,
            huff: HuffContext { table: None },
            fse: FSEContext {
                ll: None,
                ml: None,
                of: None,
            },
            offset_hist: [1, 4, 8],
            scratch_buf: vec![0; MAX_BLOCK_SIZE as usize],
            // scratch_buf: Vec::with_capacity(MAX_BLOCK_SIZE as usize),
        }
    }

    pub fn reset(&mut self, window_size: usize) {
        self.window_buf.reset(window_size);

        self.literals_idx = 0;
        // self.literals_buf.clear();

        self.sequences_buf.clear();

        self.huff = HuffContext { table: None };
        self.fse = FSEContext {
            ll: None,
            ml: None,
            of: None,
        };
        self.offset_hist = [1, 4, 8];

        // self.scratch_buf.clear();
    }
}

#[derive(Debug)]
pub struct HuffContext {
    pub table: Option<rzstd_huff0::DecodingTable>,
}

#[derive(Debug)]
pub struct FSEContext {
    pub ll: Option<rzstd_fse::DecodingTable<{ LL_DIST.table_size() }>>,
    pub ml: Option<rzstd_fse::DecodingTable<{ ML_DIST.table_size() }>>,
    pub of: Option<rzstd_fse::DecodingTable<{ OF_DIST.table_size() }>>,
}

impl<R: std::io::Read> std::fmt::Debug for Context<'_, R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Context")
            .field("window_buf", &self.window_buf)
            .field("literals_buf", &self.literals_buf)
            .field("literals_idx", &self.literals_idx)
            .field("sequences_buf", &self.sequences_buf)
            .field("huff", &self.huff)
            .field("fse", &self.fse)
            .field("offset_hist", &self.offset_hist)
            .field("scratch_buf", &self.scratch_buf)
            .finish()
    }
}
