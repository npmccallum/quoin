pub trait Device<const SIZE: usize> {
    type Error;

    fn len(&self) -> u64;
    fn get(&mut self, index: u64) -> Result<[u8; SIZE], Self::Error>;
    fn set(&mut self, index: u64, block: &[u8; SIZE]) -> Result<(), Self::Error>;

    fn blocks(size: usize) -> u64 {
        ((size + SIZE - 1) / SIZE) as u64
    }
}

impl<T: Device<SIZE>, const SIZE: usize> Device<SIZE> for &mut T {
    type Error = T::Error;

    #[inline]
    fn len(&self) -> u64 {
        (**self).len()
    }

    #[inline]
    fn get(&mut self, index: u64) -> Result<[u8; SIZE], Self::Error> {
        (**self).get(index)
    }

    #[inline]
    fn set(&mut self, index: u64, block: &[u8; SIZE]) -> Result<(), Self::Error> {
        (**self).set(index, block)
    }
}
