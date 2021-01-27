use quoin_device::Device;

use rand::Rng;

// Randomly corrupts one byte of a block during read
pub struct Corrupt<T: Device<SIZE>, const SIZE: usize> {
    device: T,
    random: rand::rngs::ThreadRng,
    odds: f64,
}

impl<T: Device<SIZE>, const SIZE: usize> Corrupt<T, SIZE> {
    pub fn new(device: T, odds: f64) -> Self {
        Self {
            device,
            random: rand::rngs::ThreadRng::default(),
            odds,
        }
    }

    pub fn set_odds(&mut self, odds: f64) {
        self.odds = odds;
    }
}

impl<T: Device<SIZE>, const SIZE: usize> Device<SIZE> for Corrupt<T, SIZE> {
    type Error = T::Error;

    #[inline]
    fn len(&self) -> u64 {
        self.device.len()
    }

    #[inline]
    fn get(&mut self, index: u64) -> Result<[u8; SIZE], Self::Error> {
        let mut block = self.device.get(index)?;

        if self.random.gen_bool(self.odds) {
            let idx = self.random.gen_range(0..block.len());
            block[idx] = self.random.gen();
        }

        Ok(block)
    }

    #[inline]
    fn set(&mut self, index: u64, block: &[u8; SIZE]) -> Result<(), Self::Error> {
        self.device.set(index, block)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use quoin_memory::Memory;

    #[test]
    fn corrupt() {
        const BLOCK: [u8; 512] = [0xff; 512];
        const TOTAL: usize = 100_000;

        let mut memory: Memory<512, 1> = Memory::default();
        memory.set(0, &BLOCK).unwrap();

        let mut corrupt = Corrupt::new(memory, 0.1);
        let mut corrupted = 0;

        for _ in 0..TOTAL {
            match corrupt.get(0).unwrap() {
                BLOCK => (),
                _ => corrupted += 1,
            }
        }

        let percent = corrupted as f64 / TOTAL as f64;
        eprintln!("corrupted: {}%", percent);
        assert!(percent > 0.09);
        assert!(percent < 0.11);
    }
}
