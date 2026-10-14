use std::borrow::Cow;

use anyhow::{Context, Result, anyhow, bail};
use openh264::OpenH264API;
use openh264::Timestamp;
use openh264::encoder::{Encoder, EncoderConfig, FrameRate, RateControlMode, UsageType, VuiConfig};
use openh264::formats::{RgbaSliceU8, YUVBuffer};

use super::{Movie, SLOT_MS};
use crate::native::render::Image;

const TIMESCALE: u32 = 1000;
const SPS: u8 = 7;
const PPS: u8 = 8;
const IDR: u8 = 5;

#[derive(Default)]
pub struct Mp4 {
    encoder: Option<Encoder>,
    size: (u32, u32),
    sps: Vec<u8>,
    pps: Vec<u8>,
    samples: Vec<Sample>,
    data: Vec<u8>,
}

struct Sample {
    size: u32,
    duration: u32,
    sync: bool,
}

impl Movie for Mp4 {
    fn add(&mut self, image: &Image, start_ms: u64, end_ms: u64) -> Result<()> {
        let (rgba, width, height) = even(image);
        let encoder = match &mut self.encoder {
            Some(encoder) => encoder,
            None => {
                self.size = (width as u32, height as u32);
                let config = EncoderConfig::new()
                    .usage_type(UsageType::ScreenContentRealTime)
                    .rate_control_mode(RateControlMode::Off)
                    .max_frame_rate(FrameRate::from_hz((1000 / SLOT_MS) as f32))
                    .skip_frames(false)
                    .adaptive_quantization(false)
                    .background_detection(false)
                    .vui(VuiConfig::bt601());
                let encoder = Encoder::with_api_config(OpenH264API::from_source(), config)
                    .map_err(|e| anyhow!("start the H.264 encoder: {e}"))?;
                self.encoder.insert(encoder)
            }
        };
        let yuv = YUVBuffer::from_rgba8_source(RgbaSliceU8::new(&rgba, (width, height)));
        let encoded = encoder
            .encode_at(&yuv, Timestamp::from_millis(start_ms))
            .map_err(|e| anyhow!("encode an H.264 frame: {e}"))?;
        let mut sample = Vec::new();
        let mut sync = false;
        for layer in (0..encoded.num_layers()).filter_map(|i| encoded.layer(i)) {
            for nal in (0..layer.nal_count()).filter_map(|i| layer.nal_unit(i)) {
                let nal = without_start_code(nal);
                match nal.first().map(|header| header & 0x1f) {
                    Some(SPS) if self.sps.is_empty() => self.sps = nal.to_vec(),
                    Some(PPS) if self.pps.is_empty() => self.pps = nal.to_vec(),
                    Some(SPS | PPS) | None => {}
                    Some(kind) => {
                        sync |= kind == IDR;
                        sample.extend_from_slice(&(nal.len() as u32).to_be_bytes());
                        sample.extend_from_slice(nal);
                    }
                }
            }
        }
        if sample.is_empty() {
            bail!("the H.264 encoder dropped the frame at {start_ms} ms");
        }
        let duration = u32::try_from(end_ms - start_ms).context("a frame of the clip lasts too long for an MP4")?;
        self.samples.push(Sample { size: sample.len() as u32, duration, sync });
        self.data.extend_from_slice(&sample);
        Ok(())
    }

    fn size(&self) -> (u32, u32) {
        self.size
    }

    fn finish(self) -> Result<Vec<u8>> {
        if self.sps.len() < 4 || self.pps.is_empty() {
            bail!("the H.264 encoder gave no parameter sets");
        }
        let ftyp = boxed(b"ftyp", &[*b"isom", 0x200u32.to_be_bytes(), *b"isom", *b"iso2", *b"avc1", *b"mp41"].concat());
        let data_offset = ftyp.len() + self.moov(0).len() + 8;
        let total = data_offset + self.data.len();
        if u32::try_from(total).is_err() {
            bail!("the clip would be a {total} byte MP4, more than an MP4 without 64-bit offsets holds");
        }
        let offset = data_offset as u32;
        let mut file = ftyp;
        file.extend_from_slice(&self.moov(offset));
        file.extend_from_slice(&boxed(b"mdat", &self.data));
        Ok(file)
    }
}

impl Mp4 {
    fn moov(&self, data_offset: u32) -> Vec<u8> {
        let duration: u32 = self.samples.iter().map(|sample| sample.duration).sum();
        let (width, height) = self.size;
        let mut mvhd = Vec::new();
        mvhd.extend_from_slice(&[0u8; 8]);
        mvhd.extend_from_slice(&TIMESCALE.to_be_bytes());
        mvhd.extend_from_slice(&duration.to_be_bytes());
        mvhd.extend_from_slice(&0x0001_0000u32.to_be_bytes());
        mvhd.extend_from_slice(&0x0100u16.to_be_bytes());
        mvhd.extend_from_slice(&[0u8; 10]);
        mvhd.extend_from_slice(&MATRIX);
        mvhd.extend_from_slice(&[0u8; 24]);
        mvhd.extend_from_slice(&2u32.to_be_bytes());

        let mut tkhd = Vec::new();
        tkhd.extend_from_slice(&[0u8; 8]);
        tkhd.extend_from_slice(&1u32.to_be_bytes());
        tkhd.extend_from_slice(&[0u8; 4]);
        tkhd.extend_from_slice(&duration.to_be_bytes());
        tkhd.extend_from_slice(&[0u8; 16]);
        tkhd.extend_from_slice(&MATRIX);
        tkhd.extend_from_slice(&(width << 16).to_be_bytes());
        tkhd.extend_from_slice(&(height << 16).to_be_bytes());

        let mut mdhd = Vec::new();
        mdhd.extend_from_slice(&[0u8; 8]);
        mdhd.extend_from_slice(&TIMESCALE.to_be_bytes());
        mdhd.extend_from_slice(&duration.to_be_bytes());
        mdhd.extend_from_slice(&UNDETERMINED_LANGUAGE.to_be_bytes());
        mdhd.extend_from_slice(&[0u8; 2]);

        let hdlr = [&[0u8; 4][..], b"vide", &[0u8; 12], b"VideoHandler\0"].concat();
        let vmhd = [0u8; 8];
        let dref = full(b"dref", 0, &[&1u32.to_be_bytes()[..], &full(b"url ", 1, &[])].concat());

        let stbl = [
            full(b"stsd", 0, &[&1u32.to_be_bytes()[..], &self.avc1()].concat()),
            full(b"stts", 0, &self.durations()),
            full(b"stss", 0, &self.syncs()),
            full(b"stsc", 0, &[1u32, 1, self.samples.len() as u32, 1].map(u32::to_be_bytes).concat()),
            full(b"stsz", 0, &self.sizes()),
            full(b"stco", 0, &[1u32, data_offset].map(u32::to_be_bytes).concat()),
        ]
        .concat();
        let minf = [full(b"vmhd", 1, &vmhd), boxed(b"dinf", &dref), boxed(b"stbl", &stbl)].concat();
        let mdia = [full(b"mdhd", 0, &mdhd), full(b"hdlr", 0, &hdlr), boxed(b"minf", &minf)].concat();
        let trak = [full(b"tkhd", 3, &tkhd), boxed(b"mdia", &mdia)].concat();
        boxed(b"moov", &[full(b"mvhd", 0, &mvhd), boxed(b"trak", &trak)].concat())
    }

    fn avc1(&self) -> Vec<u8> {
        let (width, height) = self.size;
        let mut avc1 = Vec::new();
        avc1.extend_from_slice(&[0u8; 6]);
        avc1.extend_from_slice(&1u16.to_be_bytes());
        avc1.extend_from_slice(&[0u8; 16]);
        avc1.extend_from_slice(&(width as u16).to_be_bytes());
        avc1.extend_from_slice(&(height as u16).to_be_bytes());
        avc1.extend_from_slice(&0x0048_0000u32.to_be_bytes());
        avc1.extend_from_slice(&0x0048_0000u32.to_be_bytes());
        avc1.extend_from_slice(&[0u8; 4]);
        avc1.extend_from_slice(&1u16.to_be_bytes());
        avc1.extend_from_slice(&[0u8; 32]);
        avc1.extend_from_slice(&0x0018u16.to_be_bytes());
        avc1.extend_from_slice(&(-1i16).to_be_bytes());
        let mut avcc = vec![1, self.sps[1], self.sps[2], self.sps[3], 0xff, 0xe1];
        avcc.extend_from_slice(&(self.sps.len() as u16).to_be_bytes());
        avcc.extend_from_slice(&self.sps);
        avcc.push(1);
        avcc.extend_from_slice(&(self.pps.len() as u16).to_be_bytes());
        avcc.extend_from_slice(&self.pps);
        avc1.extend_from_slice(&boxed(b"avcC", &avcc));
        boxed(b"avc1", &avc1)
    }

    fn durations(&self) -> Vec<u8> {
        let mut runs: Vec<(u32, u32)> = Vec::new();
        for sample in &self.samples {
            match runs.last_mut() {
                Some((count, duration)) if *duration == sample.duration => *count += 1,
                _ => runs.push((1, sample.duration)),
            }
        }
        let mut table = (runs.len() as u32).to_be_bytes().to_vec();
        for (count, duration) in runs {
            table.extend_from_slice(&count.to_be_bytes());
            table.extend_from_slice(&duration.to_be_bytes());
        }
        table
    }

    fn syncs(&self) -> Vec<u8> {
        let syncs: Vec<u32> =
            self.samples.iter().enumerate().filter(|(_, sample)| sample.sync).map(|(i, _)| i as u32 + 1).collect();
        let mut table = (syncs.len() as u32).to_be_bytes().to_vec();
        for number in syncs {
            table.extend_from_slice(&number.to_be_bytes());
        }
        table
    }

    fn sizes(&self) -> Vec<u8> {
        let mut table = [0u32, self.samples.len() as u32].map(u32::to_be_bytes).concat();
        for sample in &self.samples {
            table.extend_from_slice(&sample.size.to_be_bytes());
        }
        table
    }
}

const UNDETERMINED_LANGUAGE: u16 = 0x55c4;

const MATRIX: [u8; 36] = {
    let mut matrix = [0u8; 36];
    matrix[1] = 1;
    matrix[17] = 1;
    matrix[32] = 0x40;
    matrix
};

fn boxed(kind: &[u8; 4], body: &[u8]) -> Vec<u8> {
    let mut out = ((body.len() + 8) as u32).to_be_bytes().to_vec();
    out.extend_from_slice(kind);
    out.extend_from_slice(body);
    out
}

fn full(kind: &[u8; 4], flags: u32, body: &[u8]) -> Vec<u8> {
    boxed(kind, &[&(flags & 0x00ff_ffff).to_be_bytes()[..], body].concat())
}

fn without_start_code(nal: &[u8]) -> &[u8] {
    nal.strip_prefix(&[0, 0, 0, 1]).or_else(|| nal.strip_prefix(&[0, 0, 1])).unwrap_or(nal)
}

fn even(image: &Image) -> (Cow<'_, [u8]>, usize, usize) {
    let (width, height) = (image.width() as usize, image.height() as usize);
    let (even_width, even_height) = (width.next_multiple_of(2), height.next_multiple_of(2));
    if (even_width, even_height) == (width, height) {
        return (Cow::Borrowed(image.rgba()), width, height);
    }
    let mut padded = Vec::with_capacity(even_width * even_height * 4);
    for row in
        image.rgba().chunks_exact(width * 4).chain(image.rgba().rchunks_exact(width * 4).take(even_height - height))
    {
        padded.extend_from_slice(row);
        padded.extend_from_slice(&row[row.len() - 4..].repeat(even_width - width));
    }
    (Cow::Owned(padded), even_width, even_height)
}
