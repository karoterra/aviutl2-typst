use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use rustc_hash::FxHashMap;
use typst::{
    diag::{EcoString, FileError, FileResult},
    foundations::Bytes,
    syntax::{FileId, Source, VirtualPath},
};
use typst_kit::package::PackageStorage;

use crate::typst_package::PackageDownloadProgress;

pub struct FileStore {
    main: Source,
    root: Option<PathBuf>,
    slots: Mutex<FxHashMap<FileId, FileSlot>>,
    package_storage: Arc<Mutex<PackageStorage>>,
}

impl FileStore {
    pub fn new(
        source: &str,
        root: Option<PathBuf>,
        package_storage: Arc<Mutex<PackageStorage>>,
    ) -> Self {
        let path = VirtualPath::new("<main>");
        let id = FileId::new_fake(path);
        let main = Source::new(id, source.to_string());

        Self {
            main,
            root,
            slots: Mutex::new(FxHashMap::default()),
            package_storage,
        }
    }

    pub fn main(&self) -> FileId {
        self.main.id()
    }

    pub fn source(&self, id: FileId) -> FileResult<Source> {
        if id == self.main.id() {
            return Ok(self.main.clone());
        }
        let mut map = self.slots.lock().unwrap();
        let ps = self.package_storage.lock().unwrap();
        map.entry(id)
            .or_insert_with(|| FileSlot::new(id))
            .source(self.root.as_ref(), &ps)
    }

    pub fn file(&self, id: FileId) -> FileResult<Bytes> {
        let mut map = self.slots.lock().unwrap();
        let ps = self.package_storage.lock().unwrap();
        map.entry(id)
            .or_insert_with(|| FileSlot::new(id))
            .file(self.root.as_ref(), &ps)
    }
}

struct FileSlot {
    id: FileId,
    source: Option<FileResult<Source>>,
    file: Option<FileResult<Bytes>>,
}

impl FileSlot {
    fn new(id: FileId) -> Self {
        Self {
            id,
            source: None,
            file: None,
        }
    }

    fn source(
        &mut self,
        root: Option<&PathBuf>,
        package_storage: &PackageStorage,
    ) -> FileResult<Source> {
        match &self.source {
            Some(source) => source.clone(),
            None => {
                let path = resolve_path(root, self.id, package_storage)?;
                let result = fs::read_to_string(&path)
                    .map(|s| Source::new(self.id, s))
                    .map_err(|e| FileError::from_io(e, &path));
                self.source = Some(result.clone());
                result
            }
        }
    }

    fn file(
        &mut self,
        root: Option<&PathBuf>,
        package_storage: &PackageStorage,
    ) -> FileResult<Bytes> {
        match &self.file {
            Some(file) => file.clone(),
            None => {
                let path = resolve_path(root, self.id, package_storage)?;
                let result = fs::read(&path)
                    .map(Bytes::new)
                    .map_err(|e| FileError::from_io(e, &path));
                self.file = Some(result.clone());
                result
            }
        }
    }
}

fn resolve_path(
    root: Option<&PathBuf>,
    id: FileId,
    package_storage: &PackageStorage,
) -> FileResult<PathBuf> {
    if let Some(spec) = id.package() {
        let mut progress = PackageDownloadProgress { package: spec };
        let dir = package_storage.prepare_package(spec, &mut progress)?;
        id.vpath().resolve(&dir).ok_or(FileError::AccessDenied)
    } else if let Some(root) = root {
        id.vpath().resolve(root).ok_or(FileError::AccessDenied)
    } else {
        Err(FileError::Other(Some(EcoString::from(
            "Root directory is not set. Please save project.",
        ))))
    }
}
