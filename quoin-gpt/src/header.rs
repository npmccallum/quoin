use quoin_codec::codec;

use super::Range;

codec! {
    #[derive(Clone, Debug)]
    pub struct Header {
        pub signature: [u8; 8],
        pub revision: [u8; 4],
        pub size: u32,
        pub crc32: u32,
        pub reserved: u32,
        pub this_lba: u64,
        pub other_lba: u64,
        pub usable: Range,
        pub guid: [u8; 16],
        pub elba: u64,
        pub ecount: u32,
        pub esize: u32,
        pub ecrc32: u32,
    }
}

impl Header {
    pub const SIGNATURE: [u8; 8] = *b"EFI PART";
    pub const REVISION: [u8; 4] = [0, 0, 1, 0];
    pub const SIZE: usize = 92;
}
