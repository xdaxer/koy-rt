//! Karşılığı: `koy/src/Core/Settings/ValueCollections.cs`.
//!
//! İçeriğe göre eşitlik gösteren liste ve sözlük. Ayar bölümleri değer eşitliklidir; düz koleksiyonlar referansla
//! karşılaştırılırdı. Diğer modüller bunları `crate::settings::{ValueList, ValueDictionary}` olarak kullanır.

use std::ops::{Deref, DerefMut, Index};

use crate::text;

/// İçeriğe göre eşitlik gösteren liste (C# `ValueList<T>`; `List<T>`'tan türer). Sıra anlamlıdır.
/// `Vec<T>`'ye `Deref` eder: `push`, `len`, `iter`, `clear`, `remove`, `retain` vb. doğrudan kullanılır.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ValueList<T>(pub Vec<T>);

impl<T> ValueList<T> {
    pub fn new() -> Self {
        ValueList(Vec::new())
    }

    /// C# `new ValueList<T>(IEnumerable<T>)`.
    pub fn from_items(items: impl IntoIterator<Item = T>) -> Self {
        ValueList(items.into_iter().collect())
    }

    pub fn into_vec(self) -> Vec<T> {
        self.0
    }
}

impl<T> Default for ValueList<T> {
    fn default() -> Self {
        ValueList(Vec::new())
    }
}

impl<T> Deref for ValueList<T> {
    type Target = Vec<T>;
    fn deref(&self) -> &Vec<T> {
        &self.0
    }
}

impl<T> DerefMut for ValueList<T> {
    fn deref_mut(&mut self) -> &mut Vec<T> {
        &mut self.0
    }
}

impl<T> From<Vec<T>> for ValueList<T> {
    fn from(v: Vec<T>) -> Self {
        ValueList(v)
    }
}

impl<T: Clone> From<&[T]> for ValueList<T> {
    fn from(v: &[T]) -> Self {
        ValueList(v.to_vec())
    }
}

impl<T, const N: usize> From<[T; N]> for ValueList<T> {
    fn from(v: [T; N]) -> Self {
        ValueList(Vec::from(v))
    }
}

impl<const N: usize> From<[&str; N]> for ValueList<String> {
    fn from(v: [&str; N]) -> Self {
        ValueList(v.iter().map(|s| s.to_string()).collect())
    }
}

impl<T> FromIterator<T> for ValueList<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        ValueList(iter.into_iter().collect())
    }
}

impl<T> IntoIterator for ValueList<T> {
    type Item = T;
    type IntoIter = std::vec::IntoIter<T>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<'a, T> IntoIterator for &'a ValueList<T> {
    type Item = &'a T;
    type IntoIter = std::slice::Iter<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl<T> Extend<T> for ValueList<T> {
    fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        self.0.extend(iter);
    }
}

// Testlerde `assert_eq!(vec![..], settings.x)` / `assert_eq!(settings.x, ["a", "b"])` yazılabilsin.
impl<T: PartialEq> PartialEq<Vec<T>> for ValueList<T> {
    fn eq(&self, other: &Vec<T>) -> bool {
        &self.0 == other
    }
}

impl<T: PartialEq> PartialEq<ValueList<T>> for Vec<T> {
    fn eq(&self, other: &ValueList<T>) -> bool {
        self == &other.0
    }
}

impl<T: PartialEq, const N: usize> PartialEq<[T; N]> for ValueList<T> {
    fn eq(&self, other: &[T; N]) -> bool {
        self.0.as_slice() == other.as_slice()
    }
}

impl<T: PartialEq, const N: usize> PartialEq<ValueList<T>> for [T; N] {
    fn eq(&self, other: &ValueList<T>) -> bool {
        self.as_slice() == other.0.as_slice()
    }
}

impl<const N: usize> PartialEq<[&str; N]> for ValueList<String> {
    fn eq(&self, other: &[&str; N]) -> bool {
        self.0.len() == N && self.0.iter().zip(other.iter()).all(|(a, b)| a == b)
    }
}

impl<const N: usize> PartialEq<ValueList<String>> for [&str; N] {
    fn eq(&self, other: &ValueList<String>) -> bool {
        other == self
    }
}

/// İçeriğe göre eşitlik gösteren, anahtarları büyük/küçük harf duyarsız (OrdinalIgnoreCase) sözlük
/// (monitör kimlikleri gibi). C# `Dictionary` ekleme sırasını korur ve JSON'a bu sırayla yazılır; burada da
/// düz vektör olarak ekleme sırası korunur. Var olan anahtara yazmak değeri yerinde değiştirir, anahtarın ilk yazımı kalır.
/// Sıradan bağımsız eşitlik: aynı kayıtlar farklı sırada yazılsa da eşittir.
#[derive(Debug, Clone)]
pub struct ValueDictionary<T>(Vec<(String, T)>);

impl<T> ValueDictionary<T> {
    pub fn new() -> Self {
        ValueDictionary(Vec::new())
    }

    /// C# `new ValueDictionary<T>(IEnumerable<KeyValuePair>)`: `this[key] = value` ile ekler (yinelenen anahtar sonuncuyu alır).
    pub fn from_items(items: impl IntoIterator<Item = (String, T)>) -> Self {
        let mut d = ValueDictionary::new();
        for (k, v) in items {
            d.insert(&k, v);
        }
        d
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    fn position(&self, key: &str) -> Option<usize> {
        self.0.iter().position(|(k, _)| text::eq_ignore_case(k, key))
    }

    pub fn contains_key(&self, key: &str) -> bool {
        self.position(key).is_some()
    }

    pub fn get(&self, key: &str) -> Option<&T> {
        self.position(key).map(|i| &self.0[i].1)
    }

    pub fn get_mut(&mut self, key: &str) -> Option<&mut T> {
        self.position(key).map(move |i| &mut self.0[i].1)
    }

    /// `dict[key] = value`. Önceki değeri döner (varsa).
    pub fn insert(&mut self, key: &str, value: T) -> Option<T> {
        match self.position(key) {
            Some(i) => Some(std::mem::replace(&mut self.0[i].1, value)),
            None => {
                self.0.push((key.to_string(), value));
                None
            }
        }
    }

    pub fn remove(&mut self, key: &str) -> Option<T> {
        self.position(key).map(|i| self.0.remove(i).1)
    }

    pub fn clear(&mut self) {
        self.0.clear();
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &T)> {
        self.0.iter().map(|(k, v)| (k, v))
    }

    pub fn keys(&self) -> impl Iterator<Item = &String> {
        self.0.iter().map(|(k, _)| k)
    }

    pub fn values(&self) -> impl Iterator<Item = &T> {
        self.0.iter().map(|(_, v)| v)
    }

    /// Ekleme sırasındaki kayıtlar (JSON yazımı için).
    pub fn entries(&self) -> &[(String, T)] {
        &self.0
    }
}

impl<T> Default for ValueDictionary<T> {
    fn default() -> Self {
        ValueDictionary(Vec::new())
    }
}

impl<T: PartialEq> PartialEq for ValueDictionary<T> {
    fn eq(&self, other: &Self) -> bool {
        if self.len() != other.len() {
            return false;
        }
        self.0.iter().all(|(k, v)| other.get(k).is_some_and(|ov| ov == v))
    }
}

impl<T: Eq> Eq for ValueDictionary<T> {}

impl<T> Index<&str> for ValueDictionary<T> {
    type Output = T;
    fn index(&self, key: &str) -> &T {
        self.get(key).expect("anahtar sözlükte yok (KeyNotFoundException)")
    }
}

impl<T> FromIterator<(String, T)> for ValueDictionary<T> {
    fn from_iter<I: IntoIterator<Item = (String, T)>>(iter: I) -> Self {
        ValueDictionary::from_items(iter)
    }
}

impl<'a, T> IntoIterator for &'a ValueDictionary<T> {
    type Item = &'a (String, T);
    type IntoIter = std::slice::Iter<'a, (String, T)>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

/// `AppIconOverrides`: büyük/küçük harf duyarsız (C# `StringComparer.OrdinalIgnoreCase` karşılaştırıcısı).
impl crate::appearance::IconOverrideMap for ValueDictionary<String> {
    fn get_value(&self, key: &str) -> Option<&str> {
        self.get(key).map(String::as_str)
    }

    fn set_value(&mut self, key: &str, value: String) {
        self.insert(key, value);
    }

    fn remove_value(&mut self, key: &str) {
        self.remove(key);
    }
}
