use hound::{WavSpec, WavWriter};
use noisy::{noisy, GenuineRandomizer};
use num_format::{Locale, ToFormattedString};
use rand::SeedableRng;
use rand_pcg::Pcg64Mcg;
use std::collections::BTreeSet;
use std::env;
use std::fs::File;
use std::io::BufWriter;
use std::path::Path;
use std::time::Instant;
use symphonia::core::audio::{AudioBuffer, AudioBufferRef, Signal};
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

mod noisy;

type Sample = i32;

/* How many significant bits?
    no samples: 0
    all 0: 0 or 1
    all 1: 0 or 1
    some 0, some 1: 1
    some 0x4000, some 0: 31
    some 0x8000, some 0: 31
    some 0x7FFF, some 0x8001: 32
*/

struct TrackInfo {
    channels: usize,
    n_frames: u64,
    sample_rate: u32,
    bits_per_sample: u32,
}

struct Analyzer {
    track_info: TrackInfo,
    min_leading: u32,
    min_trailing0: u32,
    min_trailing1: u32,
    max_value: Sample,
    min_value: Sample,
    values: Option<BTreeSet<Sample>>,
    samples: usize,
}

impl Analyzer {
    fn new(track_info: TrackInfo, deep: bool) -> Self {
        println!("    Channels: {}", track_info.channels);
        println!("    Stored bits per sample: {}", track_info.bits_per_sample);
        Self {
            track_info,
            min_leading: Sample::BITS,
            min_trailing0: Sample::BITS,
            min_trailing1: Sample::BITS,
            max_value: 0,
            min_value: 0,
            values: if deep { Some(BTreeSet::new()) } else { None },
            samples: 0,
        }
    }
    fn process_packet(&mut self, buf: AudioBuffer<i32>) {
        for c in 0..self.track_info.channels {
            for &sample in buf.chan(c) {
                let sample: Sample = sample >> (32 - self.track_info.bits_per_sample);
                let leading = if sample.is_negative() {
                    sample.leading_ones() - 1
                } else {
                    sample.leading_zeros() - 1
                };
                self.min_leading = self.min_leading.min(leading);
                self.min_trailing0 = self.min_trailing0.min(sample.trailing_zeros());
                self.min_trailing1 = self.min_trailing1.min(sample.trailing_ones());
                self.max_value = self.max_value.max(sample);
                self.min_value = self.min_value.min(sample);
                if let Some(ref mut values) = self.values {
                    values.insert(sample);
                }
                self.samples += 1;
            }
        }
    }

    fn finalize(self) {
        let locale = &Locale::en;
        let actual = Sample::BITS - self.min_leading - self.min_trailing0.max(self.min_trailing1);
        println!("    Actual bits per sample: {}", actual);
        println!("    Trailing 0s in each sample: {}", self.min_trailing0);
        println!("    Trailing 1s in each sample: {}", self.min_trailing1);
        let expected_sample = self.track_info.n_frames * (self.track_info.channels as u64);
        println!(
            "    Expected samples over all channels: {}",
            expected_sample.to_formatted_string(locale)
        );
        println!(
            "    Streamed samples over all channels: {}",
            self.samples.to_formatted_string(locale)
        );
        if let Some(values) = self.values {
            let distinct = values.len();
            println!(
                "    Distinct samples over all channels: {}",
                distinct.to_formatted_string(locale)
            );
        }
        println!(
            "    Possible sample range: {} … {}",
            (-1 << (self.track_info.bits_per_sample - 1)).to_formatted_string(locale),
            (1 << (self.track_info.bits_per_sample - 1)).to_formatted_string(locale)
        );
        println!(
            "    Streamed sample range: {} … {}",
            self.min_value.to_formatted_string(locale),
            self.max_value.to_formatted_string(locale)
        );
    }
}

struct Noizer {
    track_info: TrackInfo,
    writers: Vec<WavWriter<BufWriter<File>>>,
    rng: GenuineRandomizer<Pcg64Mcg>,
}
impl Noizer {
    fn new(track_info: TrackInfo, input_path: &Path) -> Self {
        let spec = WavSpec {
            channels: track_info.channels.try_into().unwrap(),
            sample_rate: track_info.sample_rate,
            bits_per_sample: track_info.bits_per_sample.try_into().unwrap(),
            sample_format: hound::SampleFormat::Int,
        };
        Self {
            track_info,
            writers: (0..spec.bits_per_sample)
                .map(move |noise_bits| {
                    let mut output_path = input_path.to_path_buf();
                    output_path.set_file_name(format!(
                        "{}+{}bitnoise",
                        input_path.file_stem().unwrap().to_str().unwrap(),
                        noise_bits
                    ));
                    output_path.set_extension("wav");
                    if noise_bits == 0 {
                        println!(
                            "    Writing {} and {} noisier files",
                            output_path.to_string_lossy(),
                            spec.bits_per_sample - 1
                        );
                    }
                    WavWriter::create(output_path, spec).unwrap()
                })
                .collect(),
            rng: GenuineRandomizer(Pcg64Mcg::from_seed([68u8; 16])),
        }
    }

    fn process_packet(&mut self, buf: AudioBuffer<i32>) {
        let channels = self.track_info.channels;
        let bits_per_sample = self.track_info.bits_per_sample;
        let mut sample_iters: Vec<_> = (0..channels).map(|c| buf.chan(c).iter()).collect();
        loop {
            for sample_iter in &mut sample_iters {
                if let Some(&sample) = sample_iter.next() {
                    for (noise_bits, writer) in self.writers.iter_mut().enumerate() {
                        // First switch from symphonia's left alignment to right alignment.
                        let sample = sample >> (32 - bits_per_sample);
                        let sample = noisy(sample, noise_bits as u32, &mut self.rng);
                        writer.write_sample(sample).unwrap();
                    }
                } else {
                    return;
                }
            }
        }
    }

    fn finalize(self) {
        for writer in self.writers {
            writer.finalize().unwrap();
        }
    }
}

enum Command {
    SomeInfo,
    MostInfo,
    AllInfo,
    Noise,
}

enum Executer {
    Info(Analyzer),
    Noise(Noizer),
}

fn main() {
    let codecs = symphonia::default::get_codecs();
    let hint = Hint::new();
    let meta_opts: MetadataOptions = Default::default();
    let fmt_opts: FormatOptions = Default::default();
    let decoder_opts = DecoderOptions { verify: true };

    let mut args = env::args();
    let program = args.next().unwrap();
    let command = args.next();
    let command = if command == Some(String::from("i")) {
        Command::SomeInfo
    } else if command == Some(String::from("inf")) {
        Command::MostInfo
    } else if command == Some(String::from("info")) {
        Command::AllInfo
    } else if command == Some(String::from("noise")) {
        Command::Noise
    } else {
        eprintln!("Usage: {} (i|inf|info|noise) file1 file2…", program);
        return;
    };
    for arg in args {
        let input_path = Path::new(&arg);
        println!("{}:", input_path.to_str().unwrap());
        let instant = Instant::now();
        let src = File::open(input_path).expect("failed to open media");
        let mss = MediaSourceStream::new(Box::new(src), Default::default());
        let mut format = symphonia::default::get_probe()
            .format(&hint, mss, &fmt_opts, &meta_opts)
            .expect("unsupported format")
            .format;
        let mut tracks = format.tracks().iter().filter_map(|track| {
            codecs
                .get_codec(track.codec_params.codec)
                .map(|codec_descriptor| (track, codec_descriptor))
        });
        let (track, codec_descriptor) = tracks.next().expect("no supported audio tracks");
        for (_, codec_descriptor) in tracks {
            eprintln!("    Ignoring extra {} track!", codec_descriptor.short_name)
        }
        let track_id = track.id;
        let track_info = TrackInfo {
            bits_per_sample: track
                .codec_params
                .bits_per_sample
                .expect("unknown #bits/sample"),
            channels: track
                .codec_params
                .channels
                .expect("unknown #channels")
                .count(),
            sample_rate: track.codec_params.sample_rate.expect("unknown sample rate"),
            n_frames: track.codec_params.n_frames.expect("unknown n_frames"),
        };
        let mut decoder = (codec_descriptor.inst_func)(&track.codec_params, &decoder_opts).unwrap();
        let mut executer = match command {
            Command::SomeInfo => None,
            Command::MostInfo => Some(Executer::Info(Analyzer::new(track_info, false))),
            Command::AllInfo => Some(Executer::Info(Analyzer::new(track_info, true))),
            Command::Noise => Some(Executer::Noise(Noizer::new(track_info, input_path))),
        };
        while let Ok(packet) = format.next_packet() {
            assert!(format.metadata().is_latest());
            assert_eq!(packet.track_id(), track_id);
            match decoder.decode(&packet) {
                Ok(AudioBufferRef::S32(buf)) => match &mut executer {
                    None => (),
                    Some(Executer::Info(ref mut e)) => e.process_packet(buf.into_owned()),
                    Some(Executer::Noise(ref mut e)) => e.process_packet(buf.into_owned()),
                },
                Ok(_) => unimplemented!(),
                Err(_) => unimplemented!(),
            }
        }
        match executer {
            None => (),
            Some(Executer::Info(e)) => e.finalize(),
            Some(Executer::Noise(e)) => e.finalize(),
        }
        let secs = instant.elapsed().as_secs_f32();
        println!("    Finished in {:.1}s", secs);
    }
}
