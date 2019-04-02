use failure::Fail;
use std::fmt::Debug;

#[derive(Fail, Debug)]
#[fail(display = "number overflow")]
pub struct OverflowError<T: Debug + Send + Sync + 'static>(T);

pub trait CastFrom<T: Debug + Send + Sync>: Sized {
    fn cast_from(v: T) -> Result<Self, OverflowError<T>>;
}

pub trait CastInto<T>: Sized + Debug + Send + Sync {
    fn cast_into(self) -> Result<T, OverflowError<Self>>;
}

impl<T: Debug + Send + Sync, U> CastInto<U> for T
where
    U: CastFrom<T>,
{
    fn cast_into(self) -> Result<U, OverflowError<T>> {
        U::cast_from(self)
    }
}

#[cfg(target_pointer_width = "32")]
impl CastFrom<u64> for usize {
    #[inline]
    fn cast_from(v: u64) -> Result<Self, OverflowError<u64>> {
        use std::{u64, usize};

        if v > (usize::max_value() as u64) {
            Err(OverflowError(v))
        } else {
            Ok(v as usize)
        }
    }
}

impl CastFrom<u64> for u32 {
    #[inline]
    fn cast_from(v: u64) -> Result<Self, OverflowError<u64>> {
        if v > (u32::max_value() as u64) {
            Err(OverflowError(v))
        } else {
            Ok(v as u32)
        }
    }
}

#[cfg(target_pointer_width = "64")]
impl CastFrom<u64> for usize {
    #[inline]
    fn cast_from(v: u64) -> Result<Self, OverflowError<u64>> {
        Ok(v as usize)
    }
}

impl CastFrom<usize> for u64 {
    fn cast_from(v: usize) -> Result<Self, OverflowError<usize>> {
        Ok(v as u64)
    }
}

#[cfg(any(target_pointer_width = "64", target_pointer_width = "32"))]
impl CastFrom<u32> for usize {
    #[inline]
    fn cast_from(v: u32) -> Result<Self, OverflowError<u32>> {
        Ok(v as usize)
    }
}

#[cfg(test)]
mod test {

    use super::*;

    #[test]
    #[cfg(target_pointer_width = "64")]
    fn test_u64() {
        let x = 0u64;
        let y: u32 = x.cast_into().unwrap();

        fn cu32(v: u32) -> u32 {
            v
        }

        assert_eq!(y, 0u32);

        assert_eq!(cu32((2u64 << 30).cast_into().unwrap()), 2u32 << 30);
        assert_eq!(CastInto::<u32>::cast_into(2u64 << 32).is_err(), true);
    }

}
