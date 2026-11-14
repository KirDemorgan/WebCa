use std::{
    fs, io,
    path::{Path, PathBuf},
};

use crate::ProfileBook;

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("cannot access {path}: {source}")]
    Io { path: PathBuf, source: io::Error },
    #[error("{path} is not a valid profile file: {source}")]
    Parse { path: PathBuf, source: toml::de::Error },
}

/// Reads and writes the profile book as a TOML file.
#[derive(Debug, Clone)]
pub struct ProfileStore {
    path: PathBuf,
}

impl ProfileStore {
    #[must_use]
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Loads the book, returning an empty one when the file does not exist yet.
    pub fn load(&self) -> Result<ProfileBook, StoreError> {
        match fs::read_to_string(&self.path) {
            Ok(text) => ProfileBook::from_toml(&text)
                .map_err(|source| StoreError::Parse { path: self.path.clone(), source }),
            Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(ProfileBook::new()),
            Err(source) => Err(self.io_error(source)),
        }
    }

    /// Writes through a temporary file and a rename so a crash never leaves a truncated file.
    pub fn save(&self, book: &ProfileBook) -> Result<(), StoreError> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|source| self.io_error(source))?;
        }
        let temporary = self.path.with_extension("toml.tmp");
        fs::write(&temporary, book.to_toml()).map_err(|source| self.io_error(source))?;
        fs::rename(&temporary, &self.path).map_err(|source| {
            let _ = fs::remove_file(&temporary);
            self.io_error(source)
        })
    }

    fn io_error(&self, source: io::Error) -> StoreError {
        StoreError::Io { path: self.path.clone(), source }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CameraKey, ControlId, ControlSetting};

    fn assert_io_error(result: Result<(), StoreError>) {
        assert!(result.unwrap_err().to_string().starts_with("cannot access"));
    }

    fn sample_book() -> ProfileBook {
        let mut book = ProfileBook::new();
        book.camera_mut(&CameraKey::new("cam"))
            .active_mut()
            .set(ControlId::Sharpness, ControlSetting::manual(96));
        book
    }

    #[test]
    fn missing_file_loads_as_empty_book() {
        let dir = tempfile::tempdir().unwrap();
        let store = ProfileStore::new(dir.path().join("profiles.toml"));
        assert_eq!(store.load().unwrap(), ProfileBook::new());
    }

    #[test]
    fn save_creates_directories_and_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let store = ProfileStore::new(dir.path().join("nested").join("profiles.toml"));
        let book = sample_book();
        store.save(&book).unwrap();
        assert_eq!(store.load().unwrap(), book);
        assert!(!store.path().with_extension("toml.tmp").exists());
        store.save(&ProfileBook::new()).unwrap();
        assert_eq!(store.load().unwrap(), ProfileBook::new());
    }

    #[test]
    fn corrupt_file_reports_parse_error() {
        let dir = tempfile::tempdir().unwrap();
        let store = ProfileStore::new(dir.path().join("profiles.toml"));
        fs::write(store.path(), "version = \"one\"").unwrap();
        let err = store.load().unwrap_err();
        assert!(err.to_string().contains("is not a valid profile file"));
    }

    #[test]
    fn unreadable_path_reports_io_error() {
        let dir = tempfile::tempdir().unwrap();
        let store = ProfileStore::new(dir.path());
        let err = store.load().unwrap_err();
        assert!(err.to_string().starts_with("cannot access"));
    }

    #[test]
    fn save_fails_when_parent_is_a_file() {
        let dir = tempfile::tempdir().unwrap();
        let blocker = dir.path().join("blocker");
        fs::write(&blocker, "").unwrap();
        let store = ProfileStore::new(blocker.join("profiles.toml"));
        assert_io_error(store.save(&sample_book()));
    }

    #[test]
    fn save_fails_when_temporary_file_cannot_be_written() {
        let dir = tempfile::tempdir().unwrap();
        let store = ProfileStore::new(dir.path().join("profiles.toml"));
        fs::create_dir(store.path().with_extension("toml.tmp")).unwrap();
        assert_io_error(store.save(&sample_book()));
    }

    #[test]
    fn save_cleans_up_when_rename_fails() {
        let dir = tempfile::tempdir().unwrap();
        let store = ProfileStore::new(dir.path().join("profiles.toml"));
        fs::create_dir(store.path()).unwrap();
        fs::write(store.path().join("keep"), "").unwrap();
        assert_io_error(store.save(&sample_book()));
        assert!(!store.path().with_extension("toml.tmp").exists());
    }

    #[test]
    fn root_path_without_parent_is_handled() {
        let store = ProfileStore::new("");
        assert!(store.save(&ProfileBook::new()).is_err());
    }
}
