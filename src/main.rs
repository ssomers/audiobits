use flac::StreamReader;
use num_format::{Locale, ToFormattedString};
use std::collections::BTreeSet;
use std::env;
use std::fs::File;

fn main() {
    type Sample = i32;
    let locale = &Locale::en;
    let args: Vec<String> = env::args().collect();

    for fname in &args[1..] {
        match StreamReader::<File>::from_file(fname) {
            Ok(mut stream) => {
                let info = stream.info();
                println!("channels: {}", info.channels);
                println!("stored bits: {}", info.bits_per_sample);
                let mut min_leading = Sample::BITS;
                let mut min_trailing0 = Sample::BITS;
                let mut min_trailing1 = Sample::BITS;
                let mut samples = 0usize;
                let mut values = BTreeSet::<Sample>::new();
                for sample in stream.iter::<Sample>() {
                    let leading = if sample.is_negative() {
                        sample.leading_ones()
                    } else {
                        sample.leading_zeros()
                    };
                    min_leading = min_leading.min(leading);
                    min_trailing0 = min_trailing0.min(sample.trailing_zeros());
                    min_trailing1 = min_trailing1.min(sample.trailing_ones());
                    values.insert(sample);
                    samples += 1;
                }
                let expected = info.total_samples * u64::from(info.channels);
                let distinct = values.len();
                println!("significant: {}", Sample::BITS + 1 - min_leading);
                println!("trailing 0s: {}", min_trailing0);
                println!("trailing 1s: {}", min_trailing1);
                println!("expected samples: {}", expected.to_formatted_string(locale));
                println!("streamed samples: {}", samples.to_formatted_string(locale));
                println!("distinct samples: {}", distinct.to_formatted_string(locale));
            }
            Err(error) => println!("{}: error \"{:?}\"", fname, error),
        }
    }
}
