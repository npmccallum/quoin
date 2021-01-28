#![allow(incomplete_features)]
#![feature(const_generics)]
#![feature(const_evaluatable_checked)]

use quoin_device::Device;

use openssl::error::ErrorStack;
use openssl::hash::MessageDigest;
use openssl::memcmp;
use openssl::pkey::{PKey, Private};
use openssl::sign::Signer;

use sealed::Hash;

mod sealed {
    use super::MessageDigest;

    pub trait Hash: Copy {
        const SIZE: usize;

        fn digest(self) -> MessageDigest;
    }
}

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

#[derive(Copy, Clone, Debug)]
pub struct Sha256;

impl Hash for Sha256 {
    const SIZE: usize = 32;

    #[inline]
    fn digest(self) -> MessageDigest {
        MessageDigest::sha256()
    }
}

#[derive(Copy, Clone, Debug)]
pub struct Sha384;

impl Hash for Sha384 {
    const SIZE: usize = 48;

    #[inline]
    fn digest(self) -> MessageDigest {
        MessageDigest::sha384()
    }
}

#[derive(Copy, Clone, Debug)]
pub struct Sha512;

impl Hash for Sha512 {
    const SIZE: usize = 64;

    #[inline]
    fn digest(self) -> MessageDigest {
        MessageDigest::sha512()
    }
}

pub struct Hmac<T: Device<SIZE>, H: Hash, const SIZE: usize> {
    device: T,
    secret: PKey<Private>,
    digest: H,
}

impl<T: Device<SIZE>, H: Hash, const SIZE: usize> Hmac<T, H, SIZE> {
    #[inline]
    pub fn new<K: AsRef<[u8]>>(device: T, key: K, hash: H) -> Result<Self, Error<T::Error>> {
        assert_eq!(key.as_ref().len(), H::SIZE);
        assert_eq!(SIZE % H::SIZE, 0);
        assert!(SIZE > H::SIZE);

        Ok(Self {
            device,
            secret: PKey::hmac(key.as_ref())?,
            digest: hash,
        })
    }
}

impl<T: Device<SIZE>, H: Hash, const SIZE: usize> Device<{ SIZE - H::SIZE }> for Hmac<T, H, SIZE> {
    type Error = Error<T::Error>;

    #[inline]
    fn len(&self) -> u64 {
        self.device.len()
    }

    #[inline]
    fn get(&mut self, index: u64) -> Result<[u8; SIZE - H::SIZE], Self::Error> {
        let block = match self.device.get(index) {
            Ok(x) => x,
            Err(e) => return Err(Error::Parent(e)),
        };

        let (body, tag) = block.split_at(SIZE - H::SIZE);

        let mut signer = Signer::new(self.digest.digest(), &self.secret)?;
        signer.update(&index.to_le_bytes())?;
        signer.update(&body)?;

        let mut ver = [0u8; SIZE];
        signer.sign(&mut ver[..H::SIZE])?;

        if !memcmp::eq(&tag, &ver[..H::SIZE]) {
            return Err(Error::BlockModified);
        }

        Ok(unsafe { body.align_to::<[u8; SIZE - H::SIZE]>().1[0] })
    }

    #[inline]
    fn set(&mut self, index: u64, block: &[u8; SIZE - H::SIZE]) -> Result<(), Self::Error> {
        let mut buffer = [0u8; SIZE];
        let (body, tag) = buffer.split_at_mut(SIZE - H::SIZE);
        body.copy_from_slice(block);

        let mut signer = openssl::sign::Signer::new(self.digest.digest(), &self.secret)?;
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
        const BLOCK: [u8; 480] = [0xff; 480];
        const TOTAL: usize = 100_000;

        let key = [0; 32];

        let memory: Memory<512, 1> = Memory::default();
        let corrupt = Corrupt::new(memory, 0.1);
        let mut hs256 = Hmac::new(corrupt, &key, Sha256).unwrap();
        hs256.set(0, &BLOCK).unwrap();

        let mut corrupted = 0;
        for _ in 0..TOTAL {
            match hs256.get(0) {
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
