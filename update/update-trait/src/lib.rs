use std::ops::Deref;
use std::str::FromStr;

// Add; Remove;
pub trait UpdateTrait: Sized {
    fn update<I: Iterator<Item = String>>(
        &mut self,
        key: I,
        value: String,
    ) -> Result<(), &'static str> {
        Err("Update not declared")
    }

    fn val(s: String) -> Result<Self, &'static str> {
        Err("Val not declared")
    }

    fn clear<I: Iterator<Item = String>>(&mut self, key: I) -> Result<(), &'static str> {
        Err("Clear not declared")
    }
}

impl<T: UpdateTrait> UpdateTrait for Option<T> {
    fn update<I: Iterator<Item = String>>(
        &mut self,
        mut key: I,
        value: String,
    ) -> Result<(), &'static str> {
        if let Some(x) = self {
            x.update(key, value)
        } else if key.next().is_none() {
            Self::val(value).map(|x| *self = x)
        } else {
            Err("Cannot update value because of None on path to if")
        }
    }

    fn val(s: String) -> Result<Self, &'static str> {
        T::val(s).map(|x| Some(x))
    }

    fn clear<I: Iterator<Item = String>>(&mut self, mut key: I) -> Result<(), &'static str> {
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
    fn update<I: Iterator<Item = String>>(
        &mut self,
        mut key: I,
        value: String,
    ) -> Result<(), &'static str> {
        if key.next().is_some() {
            return Err("Update failed - too long key");
        }

        Self::val(value).map(|x| *self = x)
    }

    fn val(value: String) -> Result<Self, &'static str> {
        T::from_str(value.as_str()).map_err(|_| "Update failed - cannot parse to a primitive type")
    }
}

trait Primitive: Sized + FromStr + ToString {}

impl Primitive for bool {}

impl Primitive for String {}

impl Primitive for i32 {}

impl Primitive for u8 {}

impl Primitive for u32 {}
