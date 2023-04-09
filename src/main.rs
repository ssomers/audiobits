use flac::StreamReader;
use num_format::{Locale, ToFormattedString};
use std::collections::BTreeSet;
use std::env;
use std::fs::File;

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

    for fname in env::args().skip(1) {
        match StreamReader::<File>::from_file(fname.as_str()) {
            Ok(mut stream) => {
                let info = stream.info();
                println!("channels: {}", info.channels);
                println!("stored bits: {}", info.bits_per_sample);
                let mut min_leading = Sample::BITS;
                let mut min_trailing0 = Sample::BITS;
                let mut min_trailing1 = Sample::BITS;
                let mut max_value = 0;
                let mut min_value = 0;
                let mut values = BTreeSet::<Sample>::new();
                let mut samples = 0usize;
                for sample in stream.iter::<Sample>() {
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
                let expected = info.total_samples * u64::from(info.channels);
                let distinct = values.len();
                println!("significant: {}", Sample::BITS - min_leading);
                println!("trailing 0s: {}", min_trailing0);
                println!("trailing 1s: {}", min_trailing1);
                println!("expected samples: {}", expected.to_formatted_string(locale));
                println!("streamed samples: {}", samples.to_formatted_string(locale));
                println!("distinct samples: {}", distinct.to_formatted_string(locale));
                println!(
                    "possible sample range: {} … {}",
                    (-1 << (info.bits_per_sample - 1)).to_formatted_string(locale),
                    (1 << (info.bits_per_sample - 1)).to_formatted_string(locale)
                );
                println!(
                    "streamed sample range: {} … {}",
                    min_value.to_formatted_string(locale),
                    max_value.to_formatted_string(locale)
                );
            }
            Err(error) => println!("{}: error \"{:?}\"", fname, error),
        }
    }
}
