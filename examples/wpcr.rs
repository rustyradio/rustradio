/*! Test program for whole packet clock recovery.

This is the same as ax25-1200-rx.rs, except it has fewer options
(e.g. only supports reading from a file), and uses WPCR instead of
ZeroCrossing symbol sync.
*/
use std::path::PathBuf;

use anyhow::Result;
use structopt::StructOpt;

use rustradio::block::{Block, BlockRet};
use rustradio::blocks::*;
use rustradio::graph::Graph;
use rustradio::stream::{new_streamp, Streamp};
use rustradio::{Error, Float};

#[derive(StructOpt, Debug)]
#[structopt()]
struct Opt {
    #[structopt(short = "r")]
    read: String,

    #[structopt(long = "sample_rate", default_value = "50000")]
    sample_rate: Float,

    #[structopt(short = "o")]
    output: PathBuf,

    #[structopt(short = "v", default_value = "0")]
    verbose: usize,

    #[structopt(long = "threshold", default_value = "0.0001")]
    threshold: Float,

    #[structopt(long = "iir_alpha", default_value = "0.01")]
    iir_alpha: Float,
}

pub struct VecToStream<T> {
    src: Streamp<Vec<T>>,
    dst: Streamp<T>,
}

impl<T> VecToStream<T> {
    pub fn new(src: Streamp<Vec<T>>) -> Self {
        Self {
            src,
            dst: new_streamp(),
        }
    }
    pub fn out(&self) -> Streamp<T> {
        self.dst.clone()
    }
}

impl<T: Copy> Block for VecToStream<T> {
    fn block_name(&self) -> &'static str {
        "VecToStream"
    }
    fn work(&mut self) -> Result<BlockRet, Error> {
        let mut i = self.src.lock()?;
        if i.available() == 0 {
            return Ok(BlockRet::Noop);
        }
        let mut o = self.dst.lock()?;
        for v in i.iter() {
            o.write_slice(v);
        }
        i.clear();
        Ok(BlockRet::Ok)
    }
}

macro_rules! add_block {
    ($g:ident, $cons:expr) => {{
        let block = Box::new($cons);
        let prev = block.out();
        $g.add(block);
        prev
    }};
}

fn main() -> Result<()> {
    let opt = Opt::from_args();
    stderrlog::new()
        .module(module_path!())
        .module("rustradio")
        .quiet(false)
        .verbosity(opt.verbose)
        .timestamp(stderrlog::Timestamp::Second)
        .init()
        .unwrap();

    let samp_rate = opt.sample_rate;
    let mut g = Graph::new();

    // Read file.
    let prev = add_block![g, FileSource::new(&opt.read, false)?];

    // Filter.
    let taps = rustradio::fir::low_pass_complex(samp_rate, 20_000.0, 100.0);
    let prev = add_block![g, FftFilter::new(prev, &taps)];

    // Resample RF.
    let new_samp_rate = 50_000.0;
    let prev = add_block![
        g,
        RationalResampler::new(prev, new_samp_rate as usize, samp_rate as usize)?
    ];
    let samp_rate = new_samp_rate;

    // Tee out signal strength.
    let (prev, burst_tee) = add_block![g, Tee::new(prev)];
    let burst_tee = add_block![g, ComplexToMag2::new(burst_tee)];
    let burst_tee = add_block![
        g,
        SinglePoleIIRFilter::new(burst_tee, opt.iir_alpha)
            .ok_or(Error::new("bad IIR parameters"))?
    ];

    // Save burst stream
    /*
    let (a, burst_tee) = add_block![g, Tee::new(burst_tee)];
    g.add(Box::new(FileSink::new(a, "test.f32", rustradio::file_sink::Mode::Overwrite)?));
     */

    // Demod.
    let prev = add_block![g, QuadratureDemod::new(prev, 1.0)];
    let prev = add_block![g, Hilbert::new(prev, 65)];
    let prev = add_block![g, QuadratureDemod::new(prev, 1.0)];

    // Filter.
    let taps = rustradio::fir::low_pass(samp_rate, 2400.0, 100.0);
    let prev = add_block![g, FftFilterFloat::new(prev, &taps)];

    // Center midpoint.
    let freq1 = 1200.0;
    let freq2 = 2200.0;
    let center_freq = freq1 + (freq2 - freq1) / 2.0;
    let prev = add_block![
        g,
        AddConst::new(prev, -center_freq * 2.0 * std::f32::consts::PI / samp_rate)
    ];

    // Tag.
    let prev = add_block![
        g,
        BurstTagger::new(prev, burst_tee, opt.threshold, "burst".to_string())
    ];

    let prev = add_block![
        g,
        StreamToPdu::new(prev, "burst".to_string(), samp_rate as usize, 50)
    ];

    // Symbol sync.
    let prev = add_block![g, WpcrBuilder::new(prev).samp_rate(opt.sample_rate).build()];
    let prev = add_block![g, VecToStream::new(prev)];

    // Delay xor.
    let (a, b) = add_block![g, Tee::new(prev)];
    let delay = add_block![g, Delay::new(a, 1)];
    let prev = add_block![g, Xor::new(delay, b)];
    let prev = add_block![g, XorConst::new(prev, 1u8)];

    // Decode.
    let prev = add_block![g, HdlcDeframer::new(prev, 10, 1500)];

    // Save.
    g.add(Box::new(PduWriter::new(prev, opt.output)));

    // Run.
    let st = std::time::Instant::now();
    g.run()?;
    eprintln!("{}", g.generate_stats(st.elapsed()));
    Ok(())
}