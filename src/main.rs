use num_format::{Locale, ToFormattedString};
use std::collections::BTreeSet;
use std::env;
use std::fs::File;
use symphonia::core::audio::{AudioBufferRef, Signal};
use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

/* How many significant bits?
    no samples: 0
    all 0: 0 or 1
    all 1: 0 or 1
    some 0, some 1: 1
    some 0x4000, some 0: 31
    some 0x8000, some 0: 31
    some 0x7FFF, some 0x8001: 32
*/

fn main() {
    type Sample = i32;
    let locale = &Locale::en;
    let hint = Hint::new();
    let meta_opts: MetadataOptions = Default::default();
    let fmt_opts: FormatOptions = Default::default();
    let decoder_opts = DecoderOptions { verify: true };

    for fname in env::args().skip(1) {
        let src = File::open(fname).expect("failed to open media");
        let mss = MediaSourceStream::new(Box::new(src), Default::default());
        let probed = symphonia::default::get_probe()
            .format(&hint, mss, &fmt_opts, &meta_opts)
            .expect("unsupported format");
        let mut format = probed.format;
        let mut tracks = format
            .tracks()
            .iter()
            .filter(|t| t.codec_params.codec != CODEC_TYPE_NULL);
        let track = tracks.next().expect("no supported audio tracks");
        match tracks.count() {
            0 => {}
            n => eprintln!("Warning: picking 1 of {} supported audio tracks!", n + 1),
        }
        let track_id = track.id;
        let bits_per_sample = track
            .codec_params
            .bits_per_sample
            .expect("unknown bits_per_sample");
        let channels = track
            .codec_params
            .channels
            .expect("unknown channels")
            .count();
        let total_samples = track.codec_params.n_frames.expect("unknown n_frames");
        let mut decoder = symphonia::default::get_codecs()
            .make(&track.codec_params, &decoder_opts)
            .expect("unsupported codec");
        println!("channels: {}", channels);
        println!("stored bits: {}", bits_per_sample);
        let mut min_leading = Sample::BITS;
        let mut min_trailing0 = Sample::BITS;
        let mut min_trailing1 = Sample::BITS;
        let mut max_value = 0;
        let mut min_value = 0;
        let mut values = BTreeSet::<Sample>::new();
        let mut samples = 0usize;
        while let Ok(packet) = format.next_packet() {
            assert!(format.metadata().is_latest());
            assert_eq!(packet.track_id(), track_id);
            match decoder.decode(&packet) {
                Ok(AudioBufferRef::S32(buf)) => {
                    for c in 0..channels {
                        for &sample in buf.chan(c) {
                            let sample: Sample = sample >> (32 - bits_per_sample);
                            let leading = if sample.is_negative() {
                                sample.leading_ones() - 1
                            } else {
                                sample.leading_zeros() - 1
                            };
                            min_leading = min_leading.min(leading);
                            min_trailing0 = min_trailing0.min(sample.trailing_zeros());
                            min_trailing1 = min_trailing1.min(sample.trailing_ones());
                            max_value = max_value.max(sample);
                            min_value = min_value.min(sample);
                            values.insert(sample);
                            samples += 1;
                        }
                    }
                }
                Ok(_) => unimplemented!(),
                Err(_) => unimplemented!(),
            }
        }
        let expected = total_samples * (channels as u64);
        let distinct = values.len();
        println!("significant: {}", Sample::BITS - min_leading);
        println!("trailing 0s: {}", min_trailing0);
        println!("trailing 1s: {}", min_trailing1);
        println!("expected samples: {}", expected.to_formatted_string(locale));
        println!("streamed samples: {}", samples.to_formatted_string(locale));
        println!("distinct samples: {}", distinct.to_formatted_string(locale));
        println!(
            "possible sample range: {} … {}",
            (-1 << (bits_per_sample - 1)).to_formatted_string(locale),
            (1 << (bits_per_sample - 1)).to_formatted_string(locale)
        );
        println!(
            "streamed sample range: {} … {}",
            min_value.to_formatted_string(locale),
            max_value.to_formatted_string(locale)
        );
    }
}
