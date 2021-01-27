use quoin_device::Device;

use rand::Rng;

#[derive(Copy, Clone, Debug)]
pub enum Error<T> {
    Parent(T),
    Torn,
}

impl<T> From<T> for Error<T> {
    #[inline]
    fn from(value: T) -> Self {
        Self::Parent(value)
    }
}

// Simulates tearing during a block write
pub struct Tear<T: Device<SIZE>, const SIZE: usize> {
    device: T,
    random: rand::rngs::ThreadRng,
    odds: f64,
}

impl<T: Device<SIZE>, const SIZE: usize> Tear<T, SIZE> {
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

impl<T: Device<SIZE>, const SIZE: usize> Device<SIZE> for Tear<T, SIZE> {
    type Error = Error<T::Error>;

    #[inline]
    fn len(&self) -> u64 {
        self.device.len()
    }

    #[inline]
    fn get(&mut self, index: u64) -> Result<[u8; SIZE], Self::Error> {
        Ok(self.device.get(index)?)
    }

    #[inline]
    fn set(&mut self, index: u64, block: &[u8; SIZE]) -> Result<(), Self::Error> {
        let mut tmp = self.get(index)?;

        if !self.random.gen_bool(self.odds) {
            return Ok(self.device.set(index, block)?);
        }

        let tear = self.random.gen_range(0..tmp.len());
        tmp[..tear].copy_from_slice(&block[..tear]);
        self.device.set(index, block)?;
        Err(Error::Torn)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use quoin_memory::Memory;

    #[test]
    fn tear() {
        const BLOCK: [u8; 512] = [0xff; 512];
        const TOTAL: usize = 100_000;

        let memory: Memory<512, 1> = Memory::default();
        let mut tear = Tear::new(memory, 0.1);
        let mut torn = 0;

        for _ in 0..TOTAL {
            if tear.set(0, &BLOCK).is_err() {
                torn += 1;
            }
        }

        let percent = torn as f64 / TOTAL as f64;
        eprintln!("torn: {}%", percent);
        assert!(percent > 0.09);
        assert!(percent < 0.11);
    }
}
