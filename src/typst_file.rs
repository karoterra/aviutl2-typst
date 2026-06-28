use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex};

use aviutl2::anyhow;
use typst::{
    diag::{EcoString, FileError, FileResult},
    foundations::Bytes,
    syntax::{FileId, RootedPath, VirtualPath, VirtualRoot},
};
use typst_kit::files::{FileLoader, FsRoot};
use typst_kit::packages::SystemPackages;

pub static FAKE_MAIN_ID: LazyLock<FileId> = LazyLock::new(|| {
    FileId::unique(RootedPath::new(
        VirtualRoot::Project,
        VirtualPath::new("<text>").unwrap(),
    ))
});

pub struct TypstFileLoader {
    main_id: FileId,
    fake_main_data: Bytes,
    project: Option<FsRoot>,
    packages: Arc<Mutex<SystemPackages>>,
}

impl TypstFileLoader {
    pub fn from_text(
        text: &str,
        root: Option<PathBuf>,
        packages: Arc<Mutex<SystemPackages>>,
    ) -> Self {
        Self {
            main_id: *FAKE_MAIN_ID,
            fake_main_data: Bytes::from_string(text.to_string()),
            project: root.map(FsRoot::new),
            packages,
        }
    }

    pub fn from_file(path: &Path, packages: Arc<Mutex<SystemPackages>>) -> anyhow::Result<Self> {
        let root = path
            .parent()
            .map(PathBuf::from)
            .ok_or(anyhow::anyhow!("Failed to resolve parent dir: {path:?}"))?;
        let id = RootedPath::new(
            VirtualRoot::Project,
            VirtualPath::virtualize(&root, path).unwrap(),
        )
        .intern();

        Ok(Self {
            main_id: id,
            fake_main_data: Bytes::new([]),
            project: Some(FsRoot::new(root)),
            packages,
        })
    }

    pub fn main(&self) -> FileId {
        self.main_id
    }

    pub fn project(&self) -> Option<&FsRoot> {
        self.project.as_ref()
    }

    fn root(&self, id: FileId) -> FileResult<FsRoot> {
        Ok(match id.root() {
            VirtualRoot::Project => {
                self.project
                    .clone()
                    .ok_or(FileError::Other(Some(EcoString::from(
                        "Root directory is not set. Please save project.",
                    ))))?
            }
            VirtualRoot::Package(spec) => self.packages.lock().unwrap().obtain(spec)?,
        })
    }
}

impl FileLoader for TypstFileLoader {
    fn load(&self, id: FileId) -> FileResult<Bytes> {
        if id == *FAKE_MAIN_ID {
            Ok(self.fake_main_data.clone())
        } else {
            self.root(id)?.load(id.vpath())
        }
    }
}
