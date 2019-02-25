use std::ops::Deref;
use std::str::FromStr;

// Add; Remove;
pub trait UpdateTrait: Sized {
    fn set<I: Iterator<Item = String>>(
        &mut self,
        key: I,
        value: String,
    ) -> Result<(), &'static str> {
        Err("Update not declared")
    }

    fn val(s: String) -> Result<Self, &'static str> {
        Err("Val not declared")
    }

    fn remove<I: Iterator<Item = String>>(&mut self, key: I) -> Result<(), &'static str> {
        Err("Clear not declared")
    }
}

impl<T: UpdateTrait> UpdateTrait for Option<T> {
    fn set<I: Iterator<Item = String>>(
        &mut self,
        mut key: I,
        value: String,
    ) -> Result<(), &'static str> {
        if let Some(x) = self {
            x.set(key, value)
        } else if key.next().is_none() {
            Self::val(value).map(|x| *self = x)
        } else {
            Err("Cannot set value because of None on path to if")
        }
    }

    fn val(s: String) -> Result<Self, &'static str> {
        T::val(s).map(|x| Some(x))
    }

    fn remove<I: Iterator<Item = String>>(&mut self, mut key: I) -> Result<(), &'static str> {
        match key.next() {
            Some(_) => Err("Clear failed - too long key"),
            None => {
                *self = None;
                Ok(())
            }
        }
    }
}

impl<T: Primitive> UpdateTrait for T {
    fn set<I: Iterator<Item = String>>(
        &mut self,
        mut key: I,
        value: String,
    ) -> Result<(), &'static str> {
        if key.next().is_some() {
            return Err("Set failed - too long key");
        }

        Self::val(value).map(|x| *self = x)
    }

    fn val(value: String) -> Result<Self, &'static str> {
        T::from_str(value.as_str()).map_err(|_| "Update failed - cannot parse to a primitive type")
    }
}

macro_rules! primitive_impl {
    ($($t:ty),+) => {
        $(impl Primitive for $t {})*
    }
}

trait Primitive: Sized + FromStr + ToString {}

primitive_impl!(bool, char, i8, i16, i32, i64, isize, u8, u16, u32, u64, usize, f32, f64, String);
