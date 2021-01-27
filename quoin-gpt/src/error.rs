#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Error<T> {
    Parent(T),
    Corrupted,
    Mismatch,
    Unsupported,
    OutOfBounds,
    Conflict,
}

impl<T> From<T> for Error<T> {
    fn from(value: T) -> Self {
        Self::Parent(value)
    }
}
