#[derive(Clone, Debug)]
pub struct Rng {
    state: u64,
}

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self { state: seed.max(1) }
    }

    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x
    }

    pub fn next_f32(&mut self) -> f32 {
        let value = self.next_u64() >> 40;
        value as f32 / 16_777_216.0
    }

    pub fn range_f32(&mut self, min: f32, max: f32) -> f32 {
        min + (max - min) * self.next_f32()
    }

    pub fn range_usize(&mut self, max_exclusive: usize) -> usize {
        if max_exclusive == 0 {
            return 0;
        }
        (self.next_u64() as usize) % max_exclusive
    }

    pub fn chance(&mut self, probability: f32) -> bool {
        self.next_f32() < probability
    }
}

#[cfg(test)]
mod tests {
    use super::Rng;

    #[test]
    fn same_seed_yields_identical_stream() {
        let mut a = Rng::new(12345);
        let mut b = Rng::new(12345);
        let left: Vec<u64> = (0..64).map(|_| a.next_u64()).collect();
        let right: Vec<u64> = (0..64).map(|_| b.next_u64()).collect();
        assert_eq!(left, right);
    }

    #[test]
    fn different_seeds_diverge() {
        let mut a = Rng::new(1);
        let mut b = Rng::new(2);
        let left: Vec<u64> = (0..32).map(|_| a.next_u64()).collect();
        let right: Vec<u64> = (0..32).map(|_| b.next_u64()).collect();
        assert_ne!(left, right);
    }

    #[test]
    fn zero_seed_is_coerced_away_from_the_fixed_point() {
        // A xorshift stuck at 0 would emit only zeros; seed 0 must be lifted.
        let mut rng = Rng::new(0);
        assert_ne!(rng.next_u64(), 0);
    }

    #[test]
    fn next_f32_stays_in_unit_interval() {
        let mut rng = Rng::new(7);
        for _ in 0..10_000 {
            let value = rng.next_f32();
            assert!((0.0..1.0).contains(&value), "value out of range: {value}");
        }
    }

    #[test]
    fn range_usize_is_bounded_and_zero_safe() {
        let mut rng = Rng::new(9);
        assert_eq!(rng.range_usize(0), 0);
        for _ in 0..10_000 {
            assert!(rng.range_usize(5) < 5);
        }
    }

    #[test]
    fn range_f32_respects_bounds() {
        let mut rng = Rng::new(11);
        for _ in 0..10_000 {
            let value = rng.range_f32(-3.0, 4.0);
            assert!((-3.0..4.0).contains(&value), "value out of range: {value}");
        }
    }
}
