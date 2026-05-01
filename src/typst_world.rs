use std::path::PathBuf;
use std::sync::{Arc, LazyLock, Mutex, RwLock};

use aviutl2::{anyhow, filter::RgbaPixel, tracing};
use chrono::{DateTime, Datelike, FixedOffset, Local, Utc};
use typst::{
    Library, LibraryExt, World,
    diag::{FileResult, Warned},
    foundations::{Bytes, Datetime},
    layout::PagedDocument,
    syntax::{FileId, Source},
    text::{Font, FontBook},
    utils::LazyHash,
};
use typst_kit::fonts::{FontSlot, Fonts};
use typst_kit::package::PackageStorage;

use crate::typst_file::FileStore;
use crate::typst_package::new_storage;

pub static TYPST_ENGINE: LazyLock<RwLock<TypstEngine>> =
    LazyLock::new(|| RwLock::new(TypstEngine::new()));

pub struct TypstEngine {
    library: LazyHash<Library>,
    book: LazyHash<FontBook>,
    fonts: Vec<FontSlot>,
    project_dir: Option<PathBuf>,
    package_storage: Arc<Mutex<PackageStorage>>,
}

impl TypstEngine {
    fn new() -> Self {
        let library = Library::default();

        let fonts = Fonts::searcher()
            .include_system_fonts(true)
            .include_embedded_fonts(true)
            .search();

        let package_storage = Arc::new(Mutex::new(new_storage()));

        Self {
            library: LazyHash::new(library),
            book: LazyHash::new(fonts.book),
            fonts: fonts.fonts,
            project_dir: None,
            package_storage,
        }
    }

    pub fn set_project_dir(&mut self, dir: Option<PathBuf>) {
        self.project_dir = dir;
    }

    pub fn compile(&self, source: &str, ppt: f32) -> anyhow::Result<RenderedImage> {
        let world = self.create_world(source);
        let Warned { output, warnings } = typst::compile::<PagedDocument>(&world);
        for warning in warnings {
            tracing::warn!("Typst warning: {:?}", warning);
        }
        let document = output.map_err(|errors| {
            for error in errors {
                tracing::error!("Typst error: {:?}", error);
            }
            anyhow::anyhow!("Failed to compile Typst document")
        })?;

        if document.pages.is_empty() {
            tracing::warn!("Compiled Typst document has no pages");
            return Err(anyhow::anyhow!("Compiled Typst document has no pages"));
        }
        let page = document.pages.first().unwrap();
        let image = typst_render::render(page, ppt);
        let pixels = image
            .pixels()
            .iter()
            .map(|p| RgbaPixel {
                r: p.red(),
                g: p.green(),
                b: p.blue(),
                a: p.alpha(),
            })
            .collect::<Vec<_>>();
        let rendered_image = RenderedImage {
            width: image.width(),
            height: image.height(),
            data: pixels,
        };

        Ok(rendered_image)
    }

    fn create_world(&self, main: &str) -> TypstWorld<'_> {
        TypstWorld {
            library: &self.library,
            book: &self.book,
            fonts: &self.fonts,
            now: Utc::now(),
            file_store: FileStore::new(
                main,
                self.project_dir.clone(),
                self.package_storage.clone(),
            ),
        }
    }
}

struct TypstWorld<'a> {
    library: &'a LazyHash<Library>,
    book: &'a LazyHash<FontBook>,
    fonts: &'a [FontSlot],
    now: DateTime<Utc>,
    file_store: FileStore,
}

impl World for TypstWorld<'_> {
    fn library(&self) -> &LazyHash<Library> {
        self.library
    }

    fn book(&self) -> &LazyHash<FontBook> {
        self.book
    }

    fn main(&self) -> FileId {
        self.file_store.main()
    }

    fn source(&self, id: FileId) -> FileResult<Source> {
        tracing::info!("Loading source: {:?}", id);
        self.file_store.source(id)
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        tracing::info!("Loading file: {:?}", id);
        self.file_store.file(id)
    }

    fn font(&self, index: usize) -> Option<Font> {
        self.fonts.get(index)?.get()
    }

    fn today(&self, offset: Option<i64>) -> Option<Datetime> {
        let now = match offset {
            Some(hour) => {
                let seconds = i32::try_from(hour).ok()?.checked_mul(3600)?;
                self.now.with_timezone(&FixedOffset::east_opt(seconds)?)
            }
            None => self.now.with_timezone(&Local).fixed_offset(),
        };

        Datetime::from_ymd(
            now.year(),
            now.month().try_into().ok()?,
            now.day().try_into().ok()?,
        )
    }
}

pub struct RenderedImage {
    pub width: u32,
    pub height: u32,
    pub data: Vec<RgbaPixel>,
}
