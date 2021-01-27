use quoin_device::Device;

use std::convert::Infallible;

pub struct Memory<const SIZE: usize, const COUNT: usize>([[u8; SIZE]; COUNT]);

impl<const SIZE: usize, const COUNT: usize> Default for Memory<SIZE, COUNT> {
    fn default() -> Self {
        Self([[0; SIZE]; COUNT])
    }
}

impl<const SIZE: usize, const COUNT: usize> Device<SIZE> for Memory<SIZE, COUNT> {
    type Error = Infallible;

    #[inline]
    fn len(&self) -> u64 {
        COUNT as u64
    }

    #[inline]
    fn get(&mut self, index: u64) -> Result<[u8; SIZE], Self::Error> {
        assert!(index <= COUNT as u64);
        Ok(self.0[index as usize])
    }

    #[inline]
    fn set(&mut self, index: u64, block: &[u8; SIZE]) -> Result<(), Self::Error> {
        assert!(index <= COUNT as u64);
        self.0[index as usize].copy_from_slice(block);
        Ok(())
    }
}
