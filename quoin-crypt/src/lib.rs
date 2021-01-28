use core::mem::size_of;

use quoin_device::Device;

use openssl::error::ErrorStack;
use openssl::symm::{Cipher, Crypter, Mode};

#[derive(Clone, Debug)]
pub enum Error<T> {
    Parent(T),
    Crypto(ErrorStack),
}

impl<T> From<ErrorStack> for Error<T> {
    #[inline]
    fn from(value: ErrorStack) -> Self {
        Self::Crypto(value)
    }
}

pub struct Crypt<T: Device<SIZE>, K: AsRef<[u8]>, const SIZE: usize> {
    device: T,
    secret: K,
    cipher: Cipher,
}

impl<T: Device<SIZE>, K: AsRef<[u8]>, const SIZE: usize> Crypt<T, K, SIZE> {
    #[inline]
    pub fn new(device: T, secret: K, cipher: Cipher) -> Result<Self, Error<T::Error>> {
        assert_eq!(cipher.iv_len(), Some(size_of::<u128>()));
        assert_eq!(cipher.key_len(), secret.as_ref().len());
        assert_eq!(SIZE % cipher.block_size(), 0);

        Ok(Self {
            device,
            cipher,
            secret,
        })
    }
}

impl<T: Device<SIZE>, K: AsRef<[u8]>, const SIZE: usize> Device<SIZE> for Crypt<T, K, SIZE> {
    type Error = Error<T::Error>;

    #[inline]
    fn len(&self) -> u64 {
        self.device.len()
    }

    #[inline]
    fn get(&mut self, index: u64) -> Result<[u8; SIZE], Self::Error> {
        let ciphertext = match self.device.get(index) {
            Ok(x) => x,
            Err(e) => return Err(Error::Parent(e)),
        };

        let mut crypter = Crypter::new(
            self.cipher,
            Mode::Decrypt,
            self.secret.as_ref(),
            Some(&u128::from(index).to_le_bytes()),
        )?;

        let mut plaintext = [0u8; SIZE];
        let update = crypter.update(&ciphertext, &mut plaintext)?;
        let finalize = crypter.finalize(&mut plaintext[update..])?;
        assert_eq!(update + finalize, SIZE);

        Ok(plaintext)
    }

    #[inline]
    fn set(&mut self, index: u64, block: &[u8; SIZE]) -> Result<(), Self::Error> {
        let mut crypter = Crypter::new(
            self.cipher,
            Mode::Encrypt,
            self.secret.as_ref(),
            Some(&u128::from(index).to_le_bytes()),
        )?;

        let mut ciphertext = [0u8; SIZE];
        let update = crypter.update(block, &mut ciphertext)?;
        let finalize = crypter.finalize(&mut ciphertext[update..])?;
        assert_eq!(update + finalize, SIZE);

        match self.device.set(index, &ciphertext) {
            Err(e) => return Err(Error::Parent(e)),
            Ok(()) => Ok(()),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use quoin_memory::Memory;
    use rand::Rng;

    #[test]
    fn crypt() {
        const ZERO: [u8; 512] = [0; 512];

        let mut key = [0u8; 64];
        rand::thread_rng().fill(&mut key);

        let mut memory: Memory<512, 1> = Memory::default();

        let mut crypt = Crypt::new(&mut memory, key, Cipher::aes_256_xts()).unwrap();
        crypt.set(0, &ZERO).unwrap();

        let block = memory.get(0).unwrap();
        assert_ne!(ZERO, block);

        let mut crypt = Crypt::new(&mut memory, key, Cipher::aes_256_xts()).unwrap();
        let block = crypt.get(0).unwrap();
        assert_eq!(ZERO, block);
    }
}
