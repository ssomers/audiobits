use rand::Rng;
use std::ops::Range;

// Need this abstraction over Rng because rand::rngs::mock::StepRng
// causes gen_range to either emit zero or stall.
pub trait Randomizer {
    fn gen_range(&mut self, range: Range<i32>) -> i32;
}

pub struct GenuineRandomizer<R: Rng>(pub R);
impl<R: Rng> Randomizer for GenuineRandomizer<R> {
    fn gen_range(&mut self, range: Range<i32>) -> i32 {
        self.0.gen_range(range)
    }
}

// Replace least significant bits with noise.
pub fn noisy<R: Randomizer>(sample: i32, noise_bits: u32, rng: &mut R) -> i32 {
    (sample >> noise_bits) << noise_bits | rng.gen_range(0..(1 << noise_bits))
}

mod noisy_tests {
    use super::*;

    struct MockerMin;
    struct MockerMax;
    impl Randomizer for MockerMin {
        fn gen_range(&mut self, range: Range<i32>) -> i32 {
            range.start
        }
    }
    impl Randomizer for MockerMax {
        fn gen_range(&mut self, range: Range<i32>) -> i32 {
            range.end - 1
        }
    }

    #[test]
    fn none() {
        assert_eq!(noisy(0x0000, 0, &mut MockerMin), 0x0000);
        assert_eq!(noisy(0x0000, 0, &mut MockerMax), 0x0000);
        assert_eq!(noisy(0x0001, 0, &mut MockerMin), 0x0001);
        assert_eq!(noisy(0x0001, 0, &mut MockerMax), 0x0001);
        assert_eq!(noisy(0x7FFF, 0, &mut MockerMin), 0x7FFF);
        assert_eq!(noisy(0x7FFF, 0, &mut MockerMax), 0x7FFF);
        assert_eq!(noisy(-1, 0, &mut MockerMin), -1);
        assert_eq!(noisy(-1, 0, &mut MockerMax), -1);
    }

    #[test]
    fn one() {
        assert_eq!(noisy(0x0000, 1, &mut MockerMin), 0x0000);
        assert_eq!(noisy(0x0000, 1, &mut MockerMax), 0x0001);
        assert_eq!(noisy(0x0001, 1, &mut MockerMin), 0x0000);
        assert_eq!(noisy(0x0001, 1, &mut MockerMax), 0x0001);
        assert_eq!(noisy(0x7FFF, 1, &mut MockerMin), 0x7FFE);
        assert_eq!(noisy(0x7FFF, 1, &mut MockerMax), 0x7FFF);
        assert_eq!(noisy(-1, 1, &mut MockerMin), 0xFFFFFFFEu32 as i32);
        assert_eq!(noisy(-1, 1, &mut MockerMax), -1);
    }

    #[test]
    fn eight() {
        assert_eq!(noisy(0x0000, 8, &mut MockerMin), 0x0000);
        assert_eq!(noisy(0x0000, 8, &mut MockerMax), 0x00FF);
        assert_eq!(noisy(0x00FF, 8, &mut MockerMin), 0x0000);
        assert_eq!(noisy(0x00FF, 8, &mut MockerMax), 0x00FF);
        assert_eq!(noisy(0x7FFF, 8, &mut MockerMin), 0x7F00);
        assert_eq!(noisy(0x7FFF, 8, &mut MockerMax), 0x7FFF);
        assert_eq!(noisy(-1, 8, &mut MockerMin), 0xFFFFFF00u32 as i32);
        assert_eq!(noisy(-1, 8, &mut MockerMax), -1);
    }
}
