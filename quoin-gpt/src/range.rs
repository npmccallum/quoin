use quoin_codec::codec;

codec! {
    #[derive(Copy, Clone, Debug)]
    pub struct Range {
        pub first: u64,
        pub last: u64,
    }
}

impl Range {
    #[inline]
    pub fn includes(&self, block: u64) -> bool {
        block >= self.first && block <= self.last
    }

    #[inline]
    pub fn contains(&self, range: Range) -> bool {
        self.includes(range.first) && self.includes(range.last)
    }

    #[inline]
    pub fn overlaps(&self, range: Range) -> bool {
        self.includes(range.first) || self.includes(range.last)
    }
}
