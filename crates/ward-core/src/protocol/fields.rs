//! Strict reading of the frame objects: each field is taken by name, with type and size
//! checked, and [`Fields::finish`] refuses what is left. An invented field (a `true`
//! "reveal" flag, a generation number) refuses the whole request.

use zeroize::Zeroizing;

use super::json::{InvalidJson, Value, from_base64};
use crate::entry::Secret;

pub(crate) type R<T> = Result<T, InvalidJson>;

/// The pairs of an object not read yet.
pub(crate) struct Fields(Vec<(String, Value)>);

impl Fields {
    /// The object, refusing a repeated key (two values for the same field would allow
    /// reading one and checking another).
    pub(crate) fn from_value(v: Value) -> R<Self> {
        let c = Self::unchecked(v)?;
        c.no_duplicates()?;
        Ok(c)
    }

    /// The object without checking the keys yet (the top of the request: the role is
    /// checked before, and [`Fields::no_duplicates`] after).
    pub(crate) fn unchecked(v: Value) -> R<Self> {
        match v {
            Value::Object(p) => Ok(Self(p)),
            _ => Err(InvalidJson),
        }
    }

    /// Refuses a repeated key. The object has at most `MAX_KEYS` keys.
    pub(crate) fn no_duplicates(&self) -> R<()> {
        for (i, (k, _)) in self.0.iter().enumerate() {
            if self.0[..i].iter().any(|(j, _)| j == k) {
                return Err(InvalidJson);
            }
        }
        Ok(())
    }

    pub(crate) fn empty() -> Self {
        Self(Vec::new())
    }

    /// The pairs that are left, in the frame order.
    pub(crate) fn in_order(self) -> Vec<(String, Value)> {
        self.0
    }

    pub(crate) fn take(&mut self, k: &str) -> Option<Value> {
        let i = self.0.iter().position(|(c, _)| c == k)?;
        Some(self.0.remove(i).1)
    }

    /// Refuses a field that is left.
    pub(crate) fn finish(self) -> R<()> {
        if self.0.is_empty() {
            Ok(())
        } else {
            Err(InvalidJson)
        }
    }

    fn require(&mut self, k: &str) -> R<Value> {
        self.take(k).ok_or(InvalidJson)
    }

    pub(crate) fn text(&mut self, k: &str, max: usize) -> R<String> {
        text_of(self.require(k)?, max)
    }

    /// Optional text; absent becomes empty (the "not given" of [`crate::entry::Draft`]).
    pub(crate) fn text_or_empty(&mut self, k: &str, max: usize) -> R<String> {
        match self.take(k) {
            Some(v) => text_of(v, max),
            None => Ok(String::new()),
        }
    }

    pub(crate) fn text_opt(&mut self, k: &str, max: usize) -> R<Option<String>> {
        self.take(k).map(|v| text_of(v, max)).transpose()
    }

    pub(crate) fn secret(&mut self, k: &str, max: usize) -> R<Secret> {
        secret_of(self.require(k)?, max)
    }

    pub(crate) fn secret_opt(&mut self, k: &str, max: usize) -> R<Option<Secret>> {
        self.take(k).map(|v| secret_of(v, max)).transpose()
    }

    pub(crate) fn bytes(&mut self, k: &str, max: usize) -> R<Zeroizing<Vec<u8>>> {
        bytes_of(self.require(k)?, max)
    }

    pub(crate) fn integer(&mut self, k: &str) -> R<u64> {
        match self.require(k)? {
            Value::Integer(n) => Ok(n),
            _ => Err(InvalidJson),
        }
    }

    pub(crate) fn boolean(&mut self, k: &str) -> R<bool> {
        match self.require(k)? {
            Value::Bool(b) => Ok(b),
            _ => Err(InvalidJson),
        }
    }

    /// List of up to `max` items.
    pub(crate) fn list(&mut self, k: &str, max: usize) -> R<Vec<Value>> {
        list_of(self.require(k)?, max)
    }

    /// Optional list; absent becomes empty.
    pub(crate) fn list_or_empty(&mut self, k: &str, max: usize) -> R<Vec<Value>> {
        match self.take(k) {
            Some(v) => list_of(v, max),
            None => Ok(Vec::new()),
        }
    }

    pub(crate) fn object(&mut self, k: &str) -> R<Fields> {
        Fields::from_value(self.require(k)?)
    }

    /// Optional object; absent becomes empty.
    pub(crate) fn object_or_empty(&mut self, k: &str) -> R<Fields> {
        match self.take(k) {
            Some(v) => Fields::from_value(v),
            None => Ok(Fields::empty()),
        }
    }
}

/// Plain (non-secret) text of up to `max` bytes. Leaves the `Zeroizing` without a copy.
pub(crate) fn text_of(v: Value, max: usize) -> R<String> {
    match v {
        Value::Text(mut t) if t.len() <= max => Ok(core::mem::take(&mut *t)),
        _ => Err(InvalidJson),
    }
}

/// Bytes in base64, up to `max` decoded, in a buffer that zeroes.
pub(crate) fn bytes_of(v: Value, max: usize) -> R<Zeroizing<Vec<u8>>> {
    match v {
        Value::Text(t) => from_base64(&t, max),
        _ => Err(InvalidJson),
    }
}

/// Secret in base64 (every secret of the protocol goes like this; the base64 text zeroes
/// on drop along with the frame value).
pub(crate) fn secret_of(v: Value, max: usize) -> R<Secret> {
    let mut b = bytes_of(v, max)?;
    Ok(Secret::from_bytes(core::mem::take(&mut *b)))
}

pub(crate) fn list_of(v: Value, max: usize) -> R<Vec<Value>> {
    match v {
        Value::List(l) if l.len() <= max => Ok(l),
        _ => Err(InvalidJson),
    }
}

/// List of plain texts.
pub(crate) fn texts_of(v: Vec<Value>, max: usize) -> R<Vec<String>> {
    v.into_iter().map(|t| text_of(t, max)).collect()
}
