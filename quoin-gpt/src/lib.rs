mod entry;
mod error;
mod header;
mod range;
mod table;

pub use error::Error;

use entry::{EntriesExt, Entry};
use header::Header;
use range::Range;
use table::DeviceExt;

use std::rc::Rc;

use quoin_device::Device;
use uuid::Uuid;

pub struct Disk<T: Device<SIZE>, const SIZE: usize> {
    device: Rc<T>,
    guid: Uuid,
    usable: Range,
    entries: Vec<Entry>,
}

impl<T: Device<SIZE>, const SIZE: usize> std::fmt::Debug for Disk<T, SIZE> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        f.debug_struct("Disk")
            .field("guid", &self.guid())
            .field("partitions", &self.partitions())
            .finish()
    }
}

impl<T: Device<SIZE>, const SIZE: usize> Disk<T, SIZE> {
    pub fn load(mut device: T) -> Result<Option<Self>, Error<T::Error>> {
        assert_eq!(SIZE % Entry::SIZE, 0);

        let tail = device.len() - 1;
        let tail = device.load(tail);
        let head = device.load(1);

        let table = match (head, tail) {
            (Ok(None), Ok(None)) => return Ok(None),

            (Ok(Some(head)), Ok(Some(tail))) => {
                if head.header.guid != tail.header.guid {
                    return Err(Error::Conflict);
                }

                if head.header.ecrc32 != tail.header.ecrc32 {
                    return Err(Error::Conflict);
                }

                head
            }

            (Ok(Some(head)), ..) => head,
            (.., Ok(Some(tail))) => tail,

            (Err(head), ..) => return Err(head),
            (.., Err(tail)) => return Err(tail),
        };

        let device = Rc::new(device);
        Ok(Some(Self {
            device,
            guid: Uuid::from_bytes(table.header.guid),
            usable: table.header.usable,
            entries: table.entries,
        }))
    }

    pub fn format(mut device: T) -> Result<Self, Error<T::Error>> {
        assert_eq!(SIZE % Entry::SIZE, 0);

        let guid = Uuid::new_v4();
        device.save(*guid.as_bytes(), None, &[])?;
        Ok(Disk::load(device)?.unwrap())
    }

    pub fn guid(&self) -> Uuid {
        self.guid
    }

    pub fn partitions(&self) -> Vec<Partition<T, SIZE>> {
        self.entries
            .iter()
            .cloned()
            .map(|x| Partition {
                device: self.device.clone(),
                entry: x,
            })
            .collect()
    }

    pub fn holes(&self) -> Vec<std::ops::Range<u64>> {
        let mut current = self.usable.first..self.usable.last + 1;
        let mut holes = Vec::new();

        for entry in &self.entries {
            let next = current.start..entry.data.first;
            current = entry.data.last + 1..current.end;

            if next.start != next.end {
                holes.push(next);
            }
        }

        if current.start != current.end {
            holes.push(current);
        }

        holes
    }

    pub fn add(
        &mut self,
        kind: Uuid,
        blocks: std::ops::Range<u64>,
        attr: u64,
        name: &str,
    ) -> Result<(), Error<T::Error>> {
        // Serialize the name
        let mut buff = [0u16; 36];
        let mut i = 0;
        for c in name.encode_utf16() {
            if i > buff.len() {
                return Err(Error::OutOfBounds);
            }

            buff[i] = c;
            i += 1;
        }

        let entry = Entry {
            kind: *kind.as_bytes(),
            data: Range {
                first: blocks.start,
                last: blocks.end - 1,
            },
            guid: *Uuid::new_v4().as_bytes(),
            attr,
            name: buff,
        };

        self.entries.push(entry);

        let result = Rc::get_mut(&mut self.device).unwrap().save(
            *self.guid.as_bytes(),
            Some(self.usable),
            &self.entries[..],
        );

        if result.is_err() {
            self.entries.pop();
        }

        result
    }
}

pub struct Partition<T: Device<N>, const N: usize> {
    device: Rc<T>,
    entry: Entry,
}

impl<T: Device<SIZE>, const SIZE: usize> std::fmt::Debug for Partition<T, SIZE> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        f.debug_struct("Partition")
            .field("kind", &self.kind())
            .field("guid", &self.guid())
            .field("name", &self.name().unwrap_or_else(|_| "".into()))
            .finish()
    }
}

impl<T: Device<SIZE>, const SIZE: usize> Partition<T, SIZE> {
    #[inline]
    pub fn kind(&self) -> Uuid {
        Uuid::from_bytes(self.entry.kind)
    }

    #[inline]
    pub fn guid(&self) -> Uuid {
        Uuid::from_bytes(self.entry.guid)
    }

    #[inline]
    pub fn name(&self) -> Result<String, std::string::FromUtf16Error> {
        let len = self.entry.name.len();
        let len = self.entry.name.iter().position(|&x| x == 0).unwrap_or(len);
        String::from_utf16(&self.entry.name[..len])
    }
}

impl<T: Device<SIZE>, const SIZE: usize> Device<SIZE> for Partition<T, SIZE> {
    type Error = T::Error;

    #[inline]
    fn len(&self) -> u64 {
        self.entry.data.last - self.entry.data.first + 1
    }

    #[inline]
    fn get(&mut self, index: u64) -> Result<[u8; SIZE], Self::Error> {
        assert!(index < self.len());

        Rc::get_mut(&mut self.device)
            .unwrap()
            .get(index + self.entry.data.first)
    }

    #[inline]
    fn set(&mut self, index: u64, blocks: &[u8; SIZE]) -> Result<(), Self::Error> {
        assert!(index < self.len());

        Rc::get_mut(&mut self.device)
            .unwrap()
            .set(index + self.entry.data.first, blocks)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use quoin_memory::Memory;

    #[test]
    fn empty() {
        let device: Memory<512, 128> = Memory::default();
        assert!(Disk::load(device).unwrap().is_none());
    }

    #[test]
    fn format() {
        let mut device: Memory<512, 128> = Memory::default();
        assert!(Disk::load(&mut device).unwrap().is_none());

        let disk = Disk::format(&mut device).unwrap();
        assert_eq!(disk.holes().len(), 1);

        let disk = Disk::load(&mut device).unwrap().unwrap();
        assert_eq!(disk.holes().len(), 1);
    }

    #[test]
    fn add() {
        let mut device: Memory<512, 128> = Memory::default();
        let kind = Uuid::new_v4();

        let mut disk = Disk::format(&mut device).unwrap();
        assert_eq!(disk.holes().len(), 1);
        assert_eq!(disk.partitions().len(), 0);

        disk.add(kind, disk.holes().pop().unwrap(), 0, "foo")
            .unwrap();
        assert_eq!(disk.holes().len(), 0);
        assert_eq!(disk.partitions().len(), 1);

        let disk = Disk::load(&mut device).unwrap().unwrap();
        assert_eq!(disk.holes().len(), 0);
        assert_eq!(disk.partitions().len(), 1);

        let part = disk.partitions().pop().unwrap();
        assert_eq!(part.name().unwrap(), "foo");
        assert_eq!(part.kind(), kind);
    }

    #[test]
    fn zero_head() {
        const ZERO: [u8; 512] = [0; 512];

        let mut device: Memory<512, 128> = Memory::default();
        let disk = Disk::format(&mut device).unwrap();
        let guid = disk.guid();

        device.set(1, &ZERO).unwrap();
        assert_eq!(Disk::load(device).unwrap().unwrap().guid(), guid);
    }

    #[test]
    fn zero_tail() {
        const ZERO: [u8; 512] = [0; 512];

        let mut device: Memory<512, 128> = Memory::default();
        let disk = Disk::format(&mut device).unwrap();
        let guid = disk.guid();

        device.set(device.len() - 1, &ZERO).unwrap();
        assert_eq!(Disk::load(device).unwrap().unwrap().guid(), guid);
    }

    #[test]
    fn zero_both() {
        const ZERO: [u8; 512] = [0; 512];

        let mut device: Memory<512, 128> = Memory::default();
        Disk::format(&mut device).unwrap();

        device.set(1, &ZERO).unwrap();
        device.set(device.len() - 1, &ZERO).unwrap();
        assert!(Disk::load(device).unwrap().is_none());
    }

    #[test]
    fn corrupt_head() {
        let mut device: Memory<512, 128> = Memory::default();
        let disk = Disk::format(&mut device).unwrap();
        let guid = disk.guid();

        let mut block = device.get(1).unwrap();

        block[32] = 0;
        device.set(1, &block).unwrap();

        assert_eq!(Disk::load(device).unwrap().unwrap().guid(), guid);
    }

    #[test]
    fn corrupt_tail() {
        let mut device: Memory<512, 128> = Memory::default();
        let index = device.len() - 1;

        let disk = Disk::format(&mut device).unwrap();
        let guid = disk.guid();

        let mut block = device.get(index).unwrap();

        block[32] = 0;
        device.set(index, &block).unwrap();

        assert_eq!(Disk::load(device).unwrap().unwrap().guid(), guid);
    }

    #[test]
    fn corrupt_both() {
        let mut device: Memory<512, 128> = Memory::default();
        let index = device.len() - 1;

        Disk::format(&mut device).unwrap();

        let mut block = device.get(1).unwrap();
        block[32] = 0xff;
        device.set(1, &block).unwrap();

        let mut block = device.get(index).unwrap();
        block[32] = 0xff;
        device.set(index, &block).unwrap();

        assert_eq!(Disk::load(device).unwrap_err(), Error::Corrupted);
    }
}
