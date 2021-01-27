use super::{EntriesExt, Entry, Error, Header, Range};

use std::convert::TryInto;

use quoin_codec::Codec;
use quoin_device::Device;

use crc::crc32::checksum_ieee as crc32;

#[derive(Clone, Debug)]
pub struct Table {
    pub header: Header,
    pub entries: Vec<Entry>,
}

pub trait DeviceExt<const SIZE: usize>: Device<SIZE> {
    fn save(
        &mut self,
        guid: [u8; 16],
        usable: Option<Range>,
        entries: &[Entry],
    ) -> Result<(), Error<Self::Error>> {
        let mut ebuffer = Vec::with_capacity(<[Entry]>::MIN_SIZE);
        for e in entries {
            e.encode(&mut ebuffer).unwrap();
        }

        let ecrc32 = crc32(&ebuffer[..]);
        while ebuffer.len() < <[Entry]>::MIN_SIZE || ebuffer.len() % SIZE != 0 {
            ebuffer.push(0u8);
        }

        let eblocks = unsafe { ebuffer.align_to::<[u8; SIZE]>().1 };
        if self.len() <= 3 + eblocks.len() as u64 * 2 {
            return Err(Error::OutOfBounds);
        }

        let mrange = Range {
            first: 2 + eblocks.len() as u64,
            last: self.len() - 2 - eblocks.len() as u64,
        };

        let urange = usable.unwrap_or(mrange);
        if !mrange.contains(urange) {
            return Err(Error::OutOfBounds);
        }

        entries.validate(urange)?;

        let mut head = Header {
            signature: Header::SIGNATURE,
            revision: Header::REVISION,
            size: Header::SIZE as u32,
            crc32: 0,
            reserved: 0,
            this_lba: 1,
            other_lba: self.len() - 1,
            usable: urange,
            guid,
            elba: 2,
            ecount: entries.len().try_into().unwrap(),
            esize: Entry::SIZE as u32,
            ecrc32,
        };

        let mut tail = Header {
            signature: Header::SIGNATURE,
            revision: Header::REVISION,
            size: Header::SIZE as u32,
            crc32: 0,
            reserved: 0,
            this_lba: self.len() - 1,
            other_lba: 1,
            usable: urange,
            guid,
            elba: self.len() - 1 - eblocks.len() as u64,
            ecount: entries.len().try_into().unwrap(),
            esize: Entry::SIZE as u32,
            ecrc32,
        };

        let mut hbuf = [0u8; SIZE];
        head.encode(&mut hbuf[..]).unwrap();
        head.crc32 = crc32(&hbuf[..Header::SIZE]);
        head.encode(&mut hbuf[..]).unwrap();

        let mut tbuf = [0u8; SIZE];
        tail.encode(&mut tbuf[..]).unwrap();
        tail.crc32 = crc32(&tbuf[..Header::SIZE]);
        tail.encode(&mut tbuf[..]).unwrap();

        self.set(head.this_lba, &hbuf)?;
        for i in 0..eblocks.len() {
            self.set(head.elba + i as u64, &eblocks[i])?;
        }

        for i in 0..eblocks.len() {
            self.set(tail.elba + i as u64, &eblocks[i])?;
        }
        self.set(tail.this_lba, &tbuf)?;

        Ok(())
    }

    fn load(&mut self, index: u64) -> Result<Option<Table>, Error<Self::Error>> {
        // The range of all disk blocks
        let drange = Range {
            first: 0,
            last: self.len() - 1,
        };

        // The range of blocks that can be used by entries
        let erange = Range {
            first: drange.first + 2,
            last: drange.last - 1,
        };

        // Determine the number of entries per block
        let epb = SIZE / Entry::SIZE;

        // Determine the correct location of the "other" copy of the table
        let other = match index {
            1 => self.len() - 1,
            _ => 1,
        };

        // Load the header
        let mut block = self.get(index)?;
        let header = Header::decode(&block[..]).unwrap();

        if header.signature != Header::SIGNATURE {
            return Ok(None);
        }

        if header.revision != Header::REVISION {
            return Err(Error::Unsupported);
        }

        if header.size as usize != Header::SIZE {
            return Err(Error::Unsupported);
        }

        if header.esize as usize != Entry::SIZE {
            return Err(Error::Unsupported);
        }

        if header.reserved != 0 {
            return Err(Error::Unsupported);
        }

        let mut hdr = header.clone();
        hdr.crc32 = 0;
        hdr.encode(&mut block[..]).unwrap();
        if crc32(&block[..Header::SIZE]) != header.crc32 {
            return Err(Error::Corrupted);
        }

        if header.this_lba != index {
            return Err(Error::OutOfBounds);
        }

        if header.other_lba != other {
            return Err(Error::OutOfBounds);
        }

        // Calculate the usable range
        let block_count = (header.ecount as usize + epb - 1) / epb;
        let urange = Range {
            first: erange.first + block_count as u64,
            last: erange.last - block_count as u64,
        };

        if header.usable.first > header.usable.last {
            return Err(Error::OutOfBounds);
        }

        if !urange.contains(header.usable) {
            return Err(Error::OutOfBounds);
        }

        if !erange.includes(header.elba) {
            return Err(Error::OutOfBounds);
        }

        if block_count > 0 && urange.includes(header.elba) {
            return Err(Error::OutOfBounds);
        }

        // Load the entry blocks.
        let mut blocks = vec![[0u8; SIZE]; block_count];
        for i in 0..blocks.len() {
            blocks[i] = self.get(header.elba + i as u64)?;
        }

        // Check that the blocks haven't been modified.
        let buffer = unsafe { blocks.align_to::<u8>().1 };
        let length = header.ecount as usize * Entry::SIZE;
        if crc32(&buffer[..length]) != header.ecrc32 {
            return Err(Error::Corrupted);
        }

        // Decode the entry blocks.
        let entries: Vec<Entry> = buffer
            .chunks(Entry::SIZE)
            .map(|x| Entry::decode(x).unwrap())
            .filter(|e| e.kind != Entry::EMPTY)
            .collect();

        entries.validate(urange)?;
        Ok(Some(Table { header, entries }))
    }
}

impl<T: Device<SIZE>, const SIZE: usize> DeviceExt<SIZE> for T {}
