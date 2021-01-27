use quoin_device::Device;

use std::hash::Hasher;

use crc::crc64::{Digest, ISO};

#[derive(Copy, Clone, Debug)]
pub enum Error<T> {
    Inner(T),
    IncompatibleBlockSize,
}

impl<T> From<T> for Error<T> {
    fn from(value: T) -> Self {
        Self::Inner(value)
    }
}

pub struct Journal<T: Device<LOWER>, const LOWER: usize, const UPPER: usize> {
    device: T,
}

impl<T: Device<LOWER>, const LOWER: usize, const UPPER: usize> Journal<T, LOWER, UPPER> {
    #[inline]
    fn pull(&mut self, index: u64) -> Result<[u8; UPPER], T::Error> {
        let mut block = [0u8; UPPER];

        let blocks = unsafe { block.align_to_mut::<[u8; LOWER]>().1 };
        for i in 0..blocks.len() {
            let idx = index * blocks.len() as u64 + i as u64;
            blocks[i] = self.device.get(idx)?;
        }

        Ok(block)
    }

    #[inline]
    fn push(&mut self, index: u64, block: &[u8; UPPER]) -> Result<(), T::Error> {
        let blocks = unsafe { block.align_to::<[u8; LOWER]>().1 };
        for i in 0..blocks.len() {
            let idx = index * blocks.len() as u64 + i as u64;
            self.device.set(idx, &blocks[i])?;
        }

        Ok(())
    }

    pub fn load(device: T) -> Result<Self, T::Error> {
        assert_eq!(UPPER % LOWER, 0);

        let mut journal = Self { device };
        let meta = journal.pull(0)?;
        let data = journal.pull(1)?;

        let idx = u64::from_le_bytes(unsafe { meta.align_to::<[u8; 8]>().1[0] });
        let crc = u64::from_le_bytes(unsafe { meta.align_to::<[u8; 8]>().1[1] });

        let mut digest = Digest::new(ISO);
        digest.write(&idx.to_le_bytes());
        digest.write(&data);
        if digest.finish() == crc {
            journal.push(idx + 2, &data)?;
        }

        Ok(journal)
    }
}

impl<T: Device<LOWER>, const LOWER: usize, const UPPER: usize> Device<UPPER>
    for Journal<T, LOWER, UPPER>
{
    type Error = T::Error;

    #[inline]
    fn len(&self) -> u64 {
        self.device.len() / (UPPER / LOWER) as u64 - 2
    }

    #[inline]
    fn get(&mut self, index: u64) -> Result<[u8; UPPER], Self::Error> {
        self.pull(index + 2)
    }

    #[inline]
    fn set(&mut self, index: u64, block: &[u8; UPPER]) -> Result<(), Self::Error> {
        let mut digest = Digest::new(ISO);
        digest.write(&index.to_le_bytes());
        digest.write(block);
        let crc = digest.finish();

        let mut meta = [0u8; UPPER];
        meta[..8].copy_from_slice(&index.to_le_bytes());
        meta[8..][..8].copy_from_slice(&crc.to_le_bytes());

        self.push(0, &meta)?;
        self.push(1, block)?;
        self.push(index + 2, block)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use quoin_memory::Memory;
    use quoin_poweroff::Tear;
    use rand::Rng;

    #[test]
    fn tear() {
        let memory: Memory<512, 10> = Memory::default();
        let mut tear = Tear::new(memory, 0.1);

        let blocks = [[0x00u8; 1024], [0xffu8; 1024]];
        let mut trng = rand::thread_rng();

        let mut unwritten = 0.0;
        let mut written = 0.0;
        let mut succ = false;
        for i in 0..10_000 {
            // Load a journal object
            let mut jrnl = loop {
                if let Ok(x) = Journal::load(&mut tear) {
                    break x;
                }
            };

            // If our last block write failed.
            if i > 0 && !succ {
                // Test that *all* blocks have no tearing
                for j in 0..jrnl.len() {
                    let block = jrnl.get(j).unwrap();

                    // Track how often the block is written when failing.
                    if block == blocks[(i + 1) % 2] {
                        written += 1.0;
                    } else if block == blocks[i % 2] {
                        unwritten += 1.0;
                    } else {
                        panic!("tearing occurred!");
                    }
                }
            }

            // Write a new block
            let block = &blocks[i % 2];
            let index = trng.gen_range(0..jrnl.len());
            match jrnl.set(index, block) {
                Ok(..) => succ = true,
                Err(..) => succ = false,
            }
        }

        // The percentage of times we actually write the block when tearing
        // occurred should be ~50%.
        let percent = written / (written + unwritten);
        eprintln!("percent written: {}%", percent * 100.0);
        assert!(percent > 0.47);
        assert!(percent < 0.53);
    }
}
