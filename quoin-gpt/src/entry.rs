use super::{Error, Range};

use std::cmp::max;

use quoin_codec::codec;

use nbytes::bytes;

codec! {
    #[derive(Clone, Debug)]
    pub struct Entry {
        pub kind: [u8; 16],
        pub guid: [u8; 16],
        pub data: Range,
        pub attr: u64,
        pub name: [u16; 36],
    }
}

impl Entry {
    pub const EMPTY: [u8; 16] = [0; 16];
    pub const SIZE: usize = 128;
}

pub trait EntriesExt {
    const MIN_SIZE: usize = bytes![16; KiB];

    fn blocks(&self, size: usize) -> usize;
    fn validate<T>(&self, urange: Range) -> Result<(), Error<T>>;
}

impl EntriesExt for [Entry] {
    fn blocks(&self, size: usize) -> usize {
        let mut entry_blocks = Self::MIN_SIZE / size;

        while entry_blocks * size < max(Self::MIN_SIZE, self.len() * Entry::SIZE) {
            entry_blocks += 1;
        }

        entry_blocks
    }

    fn validate<T>(&self, urange: Range) -> Result<(), Error<T>> {
        for entry in self {
            if entry.data.first > entry.data.last {
                return Err(Error::OutOfBounds);
            }

            // Make sure entries don't claim blocks used by GPT
            if !urange.contains(entry.data) {
                return Err(Error::OutOfBounds);
            }

            // Make sure that entries don't overlap.
            for e in self {
                if core::ptr::eq(entry, e) {
                    continue;
                }

                if e.data.overlaps(entry.data) {
                    return Err(Error::OutOfBounds);
                }

                if e.guid == entry.guid {
                    return Err(Error::Conflict);
                }
            }
        }

        Ok(())
    }
}
