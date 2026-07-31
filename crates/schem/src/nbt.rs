//! A deterministic NBT value type.
//!
//! `fastnbt::Value::Compound` is a `HashMap`, whose iteration order is randomized
//! per process — re-serializing it would break the byte-identity invariant
//! (ADR-0006). [`Nbt`] mirrors the NBT tag set but backs compounds with a
//! `BTreeMap`, so serialization key order is fixed. We convert `fastnbt::Value`
//! into `Nbt` on read and serialize `Nbt` on write; the compiler's structure
//! templates are round-tripped through this type.

use std::collections::BTreeMap;

use fastnbt::{ByteArray, IntArray, LongArray, Value};
use serde::ser::{Serialize, SerializeMap, SerializeSeq, Serializer};

/// A single NBT tag, with compounds ordered deterministically.
#[derive(Debug, Clone, PartialEq)]
pub enum Nbt {
    Byte(i8),
    Short(i16),
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    String(String),
    ByteArray(Vec<i8>),
    IntArray(Vec<i32>),
    LongArray(Vec<i64>),
    List(Vec<Nbt>),
    Compound(BTreeMap<String, Nbt>),
}

impl From<Value> for Nbt {
    fn from(v: Value) -> Self {
        match v {
            Value::Byte(b) => Nbt::Byte(b),
            Value::Short(s) => Nbt::Short(s),
            Value::Int(i) => Nbt::Int(i),
            Value::Long(l) => Nbt::Long(l),
            Value::Float(f) => Nbt::Float(f),
            Value::Double(d) => Nbt::Double(d),
            Value::String(s) => Nbt::String(s),
            Value::ByteArray(a) => Nbt::ByteArray(a.into_inner()),
            Value::IntArray(a) => Nbt::IntArray(a.into_inner()),
            Value::LongArray(a) => Nbt::LongArray(a.into_inner()),
            Value::List(items) => Nbt::List(items.into_iter().map(Nbt::from).collect()),
            Value::Compound(map) => {
                Nbt::Compound(map.into_iter().map(|(k, v)| (k, Nbt::from(v))).collect())
            }
        }
    }
}

impl Serialize for Nbt {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            // serde integer/float widths select the NBT tag under fastnbt's
            // serializer (i8 -> Byte, i16 -> Short, ...).
            Nbt::Byte(v) => s.serialize_i8(*v),
            Nbt::Short(v) => s.serialize_i16(*v),
            Nbt::Int(v) => s.serialize_i32(*v),
            Nbt::Long(v) => s.serialize_i64(*v),
            Nbt::Float(v) => s.serialize_f32(*v),
            Nbt::Double(v) => s.serialize_f64(*v),
            Nbt::String(v) => s.serialize_str(v),
            // The array newtypes carry a magic field fastnbt intercepts to emit
            // the packed TAG_*_Array forms rather than a generic list.
            Nbt::ByteArray(v) => ByteArray::new(v.clone()).serialize(s),
            Nbt::IntArray(v) => IntArray::new(v.clone()).serialize(s),
            Nbt::LongArray(v) => LongArray::new(v.clone()).serialize(s),
            Nbt::List(items) => {
                let mut seq = s.serialize_seq(Some(items.len()))?;
                for it in items {
                    seq.serialize_element(it)?;
                }
                seq.end()
            }
            Nbt::Compound(map) => {
                let mut m = s.serialize_map(Some(map.len()))?;
                for (k, v) in map {
                    m.serialize_entry(k, v)?;
                }
                m.end()
            }
        }
    }
}

impl Nbt {
    /// Borrow the compound map, if this is a compound.
    pub fn as_compound(&self) -> Option<&BTreeMap<String, Nbt>> {
        match self {
            Nbt::Compound(m) => Some(m),
            _ => None,
        }
    }

    /// Borrow the string, if this is a string tag.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Nbt::String(s) => Some(s),
            _ => None,
        }
    }

    /// Read any integral tag (Byte/Short/Int/Long) as `i32`.
    pub fn as_i32(&self) -> Option<i32> {
        match self {
            Nbt::Byte(b) => Some(*b as i32),
            Nbt::Short(s) => Some(*s as i32),
            Nbt::Int(i) => Some(*i),
            Nbt::Long(l) => Some(*l as i32),
            _ => None,
        }
    }

    /// Borrow an `IntArray`'s slice.
    pub fn as_i32_array(&self) -> Option<&[i32]> {
        match self {
            Nbt::IntArray(a) => Some(a),
            _ => None,
        }
    }

    /// Borrow a `ByteArray`'s slice.
    pub fn as_byte_array(&self) -> Option<&[i8]> {
        match self {
            Nbt::ByteArray(a) => Some(a),
            _ => None,
        }
    }

    /// Borrow a list's elements.
    pub fn as_list(&self) -> Option<&[Nbt]> {
        match self {
            Nbt::List(items) => Some(items),
            _ => None,
        }
    }
}
