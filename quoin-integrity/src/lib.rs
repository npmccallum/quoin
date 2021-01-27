#![allow(incomplete_features)]
#![feature(const_generics)]
#![feature(const_evaluatable_checked)]

use quoin_device::Device;

use openssl::error::ErrorStack;
use openssl::hash::MessageDigest;
use openssl::memcmp;
use openssl::pkey::{PKey, Private};
use openssl::sign::Signer;

#[derive(Clone, Debug)]
pub enum Error<T> {
    Parent(T),
    Crypto(ErrorStack),
    BlockModified,
}

impl<T> From<ErrorStack> for Error<T> {
    #[inline]
    fn from(value: ErrorStack) -> Self {
        Self::Crypto(value)
    }
}

pub struct Hs512<T: Device<SIZE>, const SIZE: usize> {
    device: T,
    secret: PKey<Private>,
    digest: MessageDigest,
}

impl<T: Device<SIZE>, const SIZE: usize> Hs512<T, SIZE> {
    #[inline]
    pub fn new(device: T, key: &[u8]) -> Result<Self, Error<T::Error>> {
        assert_eq!(SIZE % 64, 0);
        assert!(SIZE > 64);

        Ok(Self {
            device,
            secret: PKey::hmac(key)?,
            digest: MessageDigest::sha512(),
        })
    }
}

impl<T: Device<SIZE>, const SIZE: usize> Device<{ SIZE - 64 }> for Hs512<T, SIZE> {
    type Error = Error<T::Error>;

    #[inline]
    fn len(&self) -> u64 {
        self.device.len()
    }

    #[inline]
    fn get(&mut self, index: u64) -> Result<[u8; SIZE - 64], Self::Error> {
        let block = match self.device.get(index) {
            Ok(x) => x,
            Err(e) => return Err(Error::Parent(e)),
        };

        let (body, tag) = block.split_at(SIZE - 64);
        let mut ver = [0u8; 64];

        let mut signer = Signer::new(self.digest, &self.secret)?;
        signer.update(&index.to_le_bytes())?;
        signer.update(&body)?;
        signer.sign(&mut ver)?;

        if !memcmp::eq(&tag, &ver) {
            return Err(Error::BlockModified);
        }

        Ok(unsafe { body.align_to::<[u8; SIZE - 64]>().1[0] })
    }

    #[inline]
    fn set(&mut self, index: u64, block: &[u8; SIZE - 64]) -> Result<(), Self::Error> {
        let mut buffer = [0u8; SIZE];
        let (body, tag) = buffer.split_at_mut(SIZE - 64);
        body.copy_from_slice(block);

        let mut signer = openssl::sign::Signer::new(self.digest, &self.secret)?;
        signer.update(&index.to_le_bytes())?;
        signer.update(block)?;
        signer.sign(tag)?;

        match self.device.set(index, &buffer) {
            Err(e) => return Err(Error::Parent(e)),
            Ok(()) => Ok(()),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use quoin_corrupt::Corrupt;
    use quoin_memory::Memory;

    #[test]
    fn corrupt() {
        const BLOCK: [u8; 448] = [0xff; 448];
        const TOTAL: usize = 100_000;

        let key = [0; 64];

        let memory: Memory<512, 1> = Memory::default();
        let corrupt = Corrupt::new(memory, 0.1);
        let mut hs512 = Hs512::new(corrupt, &key).unwrap();
        hs512.set(0, &BLOCK).unwrap();

        let mut corrupted = 0;
        for _ in 0..TOTAL {
            match hs512.get(0) {
                Ok(block) => assert_eq!(block, BLOCK),
                Err(Error::BlockModified) => corrupted += 1,
                _ => panic!(),
            }
        }

        let percent = corrupted as f64 / TOTAL as f64;
        eprintln!("corrupted: {}%", percent);
        assert!(percent > 0.09);
        assert!(percent < 0.11);
    }
}
