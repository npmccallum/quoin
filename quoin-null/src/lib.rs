use quoin_device::Device;

pub struct Null(());

impl<const SIZE: usize> Device<SIZE> for Null {
    type Error = std::convert::Infallible;

    #[inline]
    fn len(&self) -> u64 {
        0
    }

    #[inline]
    fn get(&mut self, _index: u64) -> Result<[u8; SIZE], Self::Error> {
        Ok([0; SIZE])
    }

    #[inline]
    fn set(&mut self, _index: u64, _block: &[u8; SIZE]) -> Result<(), Self::Error> {
        Ok(())
    }
}
