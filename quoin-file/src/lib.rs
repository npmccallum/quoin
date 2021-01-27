use quoin_device::Device;

use std::convert::TryFrom;
use std::io::{ErrorKind, Write};
use std::os::raw::c_uint;
use std::os::unix::fs::FileExt;

use iocuddle::*;

const BLOCK: Group = Group::new(0x12);
//const BLKGETSIZE: Ioctl<Read, &c_long> = unsafe { BLOCK.none(96) };
//const BLKSSZGET: Ioctl<Read, &c_uint> = unsafe { BLOCK.none(104) };
//const BLKBSZGET: Ioctl<Read, &c_uint> = unsafe { BLOCK.read::<usize>(112).lie() };
const BLKGETSIZE64: Ioctl<Read, &u64> = unsafe { BLOCK.read::<usize>(114).lie() };
const BLKPBSZGET: Ioctl<Read, &c_uint> = unsafe { BLOCK.none(123) };

pub struct File<const SIZE: usize> {
    file: std::fs::File,
    size: u64,
}

impl<const SIZE: usize> TryFrom<std::fs::File> for File<SIZE> {
    type Error = std::io::Error;

    fn try_from(mut file: std::fs::File) -> Result<Self, Self::Error> {
        let size = match BLKPBSZGET.ioctl(&mut file) {
            Ok((_, phys_block_size)) => {
                if phys_block_size as usize != SIZE {
                    return Err(ErrorKind::InvalidInput.into());
                }

                BLKGETSIZE64.ioctl(&mut file)?.1 / SIZE as u64
            }

            Err(e) if e.kind() == ErrorKind::Other => file.metadata()?.len() / SIZE as u64,
            Err(e) => return Err(e),
        };

        Ok(Self { file, size })
    }
}

impl<const SIZE: usize> Device<SIZE> for File<SIZE> {
    type Error = std::io::Error;

    #[inline]
    fn len(&self) -> u64 {
        self.size
    }

    #[inline]
    fn set(&mut self, index: u64, block: &[u8; SIZE]) -> Result<(), Self::Error> {
        assert!(index < self.size);

        self.file.write_all_at(block, index * SIZE as u64)?;
        self.file.flush()
    }

    #[inline]
    fn get(&mut self, index: u64) -> Result<[u8; SIZE], Self::Error> {
        assert!(index < self.size);

        let mut block = [0; SIZE];
        self.file.read_exact_at(&mut block, index * SIZE as u64)?;
        Ok(block)
    }
}
