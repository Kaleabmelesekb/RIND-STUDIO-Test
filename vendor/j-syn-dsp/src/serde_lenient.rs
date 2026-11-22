//! Forward- and backward-compatible `serde` deserialization helpers shared across DSP state
//! and preset JSON serialization (tolerates unknown future enum variants, array length changes,
//! explicit `null` values, and non-finite floats).

use std::fmt;
use std::marker::PhantomData;

use serde::de::value::{Error as DeValueError, StrDeserializer};
use serde::de::{Deserializer, IgnoredAny, MapAccess, SeqAccess, Visitor};
use serde::Deserialize;

/// Returns `val.clamp(min, max)` when finite, or `fallback` on `NaN`/infinity.
#[inline]
pub fn finite_clamped(val: f32, fallback: f32, min: f32, max: f32) -> f32 {
    if val.is_finite() {
        val.clamp(min, max)
    } else {
        fallback
    }
}

/// Deserializes `Option<T>` as `T::default()` when JSON explicitly contains `null`.
pub fn default_on_null<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de> + Default,
{
    Option::<T>::deserialize(deserializer).map(|opt| opt.unwrap_or_default())
}

struct KnownVariantVisitor<T>(PhantomData<T>);

impl<'de, T: Deserialize<'de>> Visitor<'de> for KnownVariantVisitor<T> {
    type Value = Option<T>;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("a known enum variant string or unknown future value")
    }

    fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Self::Value, E> {
        Ok(T::deserialize(StrDeserializer::<DeValueError>::new(v)).ok())
    }

    fn visit_unit<E: serde::de::Error>(self) -> Result<Self::Value, E> {
        Ok(None)
    }

    fn visit_none<E: serde::de::Error>(self) -> Result<Self::Value, E> {
        Ok(None)
    }

    fn visit_bool<E: serde::de::Error>(self, _v: bool) -> Result<Self::Value, E> {
        Ok(None)
    }

    fn visit_i64<E: serde::de::Error>(self, _v: i64) -> Result<Self::Value, E> {
        Ok(None)
    }

    fn visit_u64<E: serde::de::Error>(self, _v: u64) -> Result<Self::Value, E> {
        Ok(None)
    }

    fn visit_f64<E: serde::de::Error>(self, _v: f64) -> Result<Self::Value, E> {
        Ok(None)
    }

    fn visit_some<D: Deserializer<'de>>(self, deserializer: D) -> Result<Self::Value, D::Error> {
        deserializer.deserialize_any(self)
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
        while seq.next_element::<IgnoredAny>()?.is_some() {}
        Ok(None)
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
        while map.next_entry::<IgnoredAny, IgnoredAny>()?.is_some() {}
        Ok(None)
    }
}

/// Deserializes a unit enum variant into `Some(T)` if recognized, or `None` if unknown/null/newer.
pub fn known_variant<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    deserializer.deserialize_any(KnownVariantVisitor(PhantomData))
}

struct FixedArrayVisitor<T, const N: usize>(PhantomData<T>);

impl<'de, T, const N: usize> Visitor<'de> for FixedArrayVisitor<T, N>
where
    T: Deserialize<'de> + Default,
{
    type Value = [T; N];

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("a sequence or null")
    }

    fn visit_unit<E: serde::de::Error>(self) -> Result<Self::Value, E> {
        Ok(std::array::from_fn(|_| T::default()))
    }

    fn visit_none<E: serde::de::Error>(self) -> Result<Self::Value, E> {
        Ok(std::array::from_fn(|_| T::default()))
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
        let mut out: [T; N] = std::array::from_fn(|_| T::default());
        for slot in out.iter_mut() {
            match seq.next_element()? {
                Some(v) => *slot = v,
                None => return Ok(out),
            }
        }
        while seq.next_element::<IgnoredAny>()?.is_some() {}
        Ok(out)
    }
}

/// Deserializes a sequence of arbitrary length (`< N`, `N`, `> N`, or `null`) into `[T; N]`,
/// padding missing trailing elements with `T::default()` and ignoring excess elements.
pub fn fixed_array<'de, D, T, const N: usize>(deserializer: D) -> Result<[T; N], D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de> + Default,
{
    deserializer.deserialize_any(FixedArrayVisitor(PhantomData))
}
