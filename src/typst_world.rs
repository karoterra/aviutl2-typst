use std::path::PathBuf;
use std::sync::LazyLock;

use aviutl2::{anyhow, filter::RgbaPixel, tracing};
use chrono::{DateTime, Datelike, FixedOffset, Local, Utc};
use typst::{
    Library, LibraryExt, World,
    diag::{FileError, FileResult, Warned},
    foundations::{Bytes, Datetime},
    layout::PagedDocument,
    syntax::{FileId, Source, VirtualPath},
    text::{Font, FontBook},
    utils::LazyHash,
};
use typst_kit::fonts::{FontSlot, Fonts};

pub static TYPST_ENGINE: LazyLock<Option<TypstEngine>> = LazyLock::new(|| {
    TypstEngine::new()
        .map_err(|e| {
            tracing::error!("Failed to initialize TypstEngine: {:?}", e);
        })
        .ok()
});

pub struct TypstEngine {
    library: LazyHash<Library>,
    book: LazyHash<FontBook>,
    fonts: Vec<FontSlot>,
}

impl TypstEngine {
    fn new() -> anyhow::Result<Self> {
        let library = Library::default();

        let fonts = Fonts::searcher()
            .include_system_fonts(true)
            .include_embedded_fonts(true)
            .search();

        Ok(Self {
            library: LazyHash::new(library),
            book: LazyHash::new(fonts.book),
            fonts: fonts.fonts,
        })
    }

    pub fn compile(&self, source: &str, ppt: f32) -> anyhow::Result<RenderedImage> {
        let path = VirtualPath::new("<main>");
        let main_file_id = FileId::new_fake(path);
        let main_source = Source::new(main_file_id, source.to_string());
        let world = self.create_world(main_source);
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

    fn create_world(&self, main: Source) -> TypstWorld<'_> {
        TypstWorld {
            library: &self.library,
            book: &self.book,
            fonts: &self.fonts,
            main,
            now: Utc::now(),
        }
    }
}

struct TypstWorld<'a> {
    library: &'a LazyHash<Library>,
    book: &'a LazyHash<FontBook>,
    fonts: &'a [FontSlot],
    main: Source,
    now: DateTime<Utc>,
}

impl World for TypstWorld<'_> {
    fn library(&self) -> &LazyHash<Library> {
        self.library
    }

    fn book(&self) -> &LazyHash<FontBook> {
        self.book
    }

    fn main(&self) -> FileId {
        self.main.id()
    }

    fn source(&self, id: FileId) -> FileResult<Source> {
        tracing::info!("Loading source: {:?}", id);
        if id == self.main.id() {
            return Ok(self.main.clone());
        }
        Err(FileError::NotFound(PathBuf::from(
            id.vpath().as_rooted_path(),
        )))
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        tracing::info!("Loading file: {:?}", id);
        Err(FileError::NotFound(PathBuf::from(
            id.vpath().as_rooted_path(),
        )))
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
