use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// Parse results kept between scans, reused while a file's validator (typically its size and
/// modification time) is unchanged. Rescans then only re-read files an agent has written to.
pub struct ParseCache<V, T> {
    entries: Mutex<HashMap<PathBuf, (V, T)>>,
}

impl<V, T> Default for ParseCache<V, T> {
    fn default() -> Self {
        Self {
            entries: Mutex::new(HashMap::new()),
        }
    }
}

impl<V: PartialEq + Clone, T: Clone> ParseCache<V, T> {
    pub fn get_or_parse<E>(
        &self,
        path: &Path,
        validator: V,
        parse: impl FnOnce() -> Result<T, E>,
    ) -> Result<T, E> {
        if let Some((cached, value)) = self.entries.lock().unwrap().get(path)
            && *cached == validator
        {
            return Ok(value.clone());
        }
        let value = parse()?;
        self.entries
            .lock()
            .unwrap()
            .insert(path.to_path_buf(), (validator, value.clone()));
        Ok(value)
    }

    /// Drops entries for files the latest scan didn't see, so the cache can't grow forever.
    pub fn retain(&self, seen: &HashSet<PathBuf>) {
        self.entries
            .lock()
            .unwrap()
            .retain(|path, _| seen.contains(path));
    }

    pub fn len(&self) -> usize {
        self.entries.lock().unwrap().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;

    #[test]
    fn reuses_results_until_the_validator_changes() {
        let cache: ParseCache<u64, String> = ParseCache::default();
        let parses = Cell::new(0);
        let parse = |value: &str| {
            parses.set(parses.get() + 1);
            Ok::<_, ()>(value.to_string())
        };
        let path = Path::new("/s.jsonl");

        assert_eq!(
            cache.get_or_parse(path, 1, || parse("first")).unwrap(),
            "first"
        );
        assert_eq!(
            cache.get_or_parse(path, 1, || parse("ignored")).unwrap(),
            "first"
        );
        assert_eq!(
            cache.get_or_parse(path, 2, || parse("second")).unwrap(),
            "second"
        );
        assert_eq!(parses.get(), 2);
    }

    #[test]
    fn failed_parses_are_not_cached() {
        let cache: ParseCache<u64, String> = ParseCache::default();
        let path = Path::new("/s.jsonl");
        assert!(
            cache
                .get_or_parse(path, 1, || Err::<String, _>("boom"))
                .is_err()
        );
        assert!(cache.is_empty());
    }

    #[test]
    fn retain_drops_unseen_files() {
        let cache: ParseCache<u64, u64> = ParseCache::default();
        for name in ["/a", "/b"] {
            cache
                .get_or_parse(Path::new(name), 1, || Ok::<_, ()>(1))
                .unwrap();
        }
        cache.retain(&HashSet::from([PathBuf::from("/a")]));
        assert_eq!(cache.len(), 1);
    }
}
