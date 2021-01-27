use std::mem::MaybeUninit;

pub trait Codec: Sized {
    fn decode(reader: impl std::io::Read) -> std::io::Result<Self>;
    fn encode(&self, writer: impl std::io::Write) -> std::io::Result<()>;
}

impl<T: Codec, const N: usize> Codec for [T; N] {
    #[inline]
    fn decode(mut reader: impl std::io::Read) -> std::io::Result<Self> {
        let mut data = MaybeUninit::<[T; N]>::uninit();

        for i in 0..N {
            let x = T::decode(&mut reader)?;
            unsafe {
                (*data.as_mut_ptr())[i] = x;
            }
        }

        Ok(unsafe { data.assume_init() })
    }

    #[inline]
    fn encode(&self, mut writer: impl std::io::Write) -> std::io::Result<()> {
        for item in self {
            item.encode(&mut writer)?;
        }

        Ok(())
    }
}

macro_rules! implint {
    ($($t:ident)+) => {
        $(
            impl Codec for $t {
                #[inline]
                fn decode(mut reader: impl std::io::Read) -> std::io::Result<Self> {
                    let mut bytes = Self::default().to_ne_bytes();
                    reader.read_exact(bytes.as_mut())?;
                    Ok(Self::from_le_bytes(bytes))
                }

                #[inline]
                fn encode(&self, mut writer: impl std::io::Write) -> std::io::Result<()> {
                    writer.write_all(&self.to_le_bytes())
                }
            }
        )+
    };
}

implint! {
    u8 u16 u32 u64 u128
    i8 i16 i32 i64 i128
}

#[macro_export]
macro_rules! codec {
    () => {};

    (
        $(#[$($sattr:meta)+])*
        $vis:vis struct $name:ident {
            $(
                $(#[$($fattr:meta)+])*
                $v:vis $field:ident: $kind:ty
            ),* $(,)?
        }
        $($next:tt)*
    ) => {
        $(#[$($sattr)+])*
        $vis struct $name {
            $(
                $(#[$($fattr)+])*
                $v $field: $kind
            ),*
        }

        impl $crate::Codec for $name {
            fn decode(mut reader: impl std::io::Read) -> std::io::Result<Self> {
                Ok(Self {
                    $($field: <$kind>::decode(&mut reader)?),*
                })
            }

            fn encode(&self, mut writer: impl std::io::Write) -> std::io::Result<()> {
                $(self.$field.encode(&mut writer)?;)*
                Ok(())
            }
        }

        codec! { $($next)* }
    };
}
