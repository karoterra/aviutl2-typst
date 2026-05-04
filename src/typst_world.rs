use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex, RwLock};

use aviutl2::{anyhow, filter::RgbaPixel, tracing};
use chrono::{DateTime, Datelike, FixedOffset, Local, Utc};
use codespan_reporting::diagnostic::{Diagnostic, Label};
use typst::ecow::eco_format;
use typst::syntax::VirtualPath;
use typst::{
    Library, LibraryExt, World, WorldExt,
    diag::{FileResult, Severity, SourceDiagnostic, Warned},
    foundations::{Bytes, Datetime},
    layout::{Page, PagedDocument},
    syntax::{FileId, Lines, Source, Span},
    text::{Font, FontBook},
    utils::LazyHash,
};
use typst_kit::fonts::{FontSlot, Fonts};
use typst_kit::package::PackageStorage;

use crate::typst_file::FileStore;
use crate::typst_package::new_storage;

type CodespanError = codespan_reporting::files::Error;
type CodespanResult<T> = Result<T, CodespanError>;

static FAKE_MAIN_PATH: &str = "<text>";

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

    pub fn compile_text(&self, source: &str) -> anyhow::Result<PagedDocument> {
        let vpath = VirtualPath::new(FAKE_MAIN_PATH);
        let id = FileId::new_fake(vpath);
        let main = Source::new(id, source.to_string());
        let world = self.create_world(main, self.project_dir.clone());
        self.compile(&world)
    }

    pub fn compile_file(&self, path: &Path) -> anyhow::Result<PagedDocument> {
        let source = std::fs::read_to_string(path)?;
        let root = path
            .parent()
            .map(PathBuf::from)
            .ok_or(anyhow::anyhow!("Failed to resolve parent dir: {path:?}"))?;
        let vpath = VirtualPath::within_root(path, &root)
            .ok_or(anyhow::anyhow!("Failed to resolve parent dir: {path:?}"))?;
        let id = FileId::new_fake(vpath);
        let main = Source::new(id, source);
        let world = self.create_world(main, Some(root));
        self.compile(&world)
    }

    fn compile(&self, world: &TypstWorld) -> anyhow::Result<PagedDocument> {
        let Warned { output, warnings } = typst::compile::<PagedDocument>(&world);

        match output {
            Ok(doc) => {
                print_diagnostics(world, &[], &warnings)
                    .map_err(|e| anyhow::anyhow!("Failed to print diagnostics: {e}"))?;
                Ok(doc)
            }
            Err(errors) => {
                print_diagnostics(world, &errors, &warnings)
                    .map_err(|e| anyhow::anyhow!("Failed to print diagnostics: {e}"))?;
                Err(anyhow::anyhow!("Failed to compile Typst document"))
            }
        }
    }

    fn create_world(&self, main: Source, root: Option<PathBuf>) -> TypstWorld<'_> {
        TypstWorld {
            library: &self.library,
            book: &self.book,
            fonts: &self.fonts,
            now: Utc::now(),
            file_store: FileStore::new(main, root, self.package_storage.clone()),
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
        tracing::debug!("Loading source: {:?}", id);
        self.file_store.source(id)
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        tracing::debug!("Loading file: {:?}", id);
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

impl<'a> codespan_reporting::files::Files<'a> for TypstWorld<'_> {
    type FileId = FileId;
    type Name = String;
    type Source = Lines<String>;

    fn name(&'a self, id: Self::FileId) -> CodespanResult<Self::Name> {
        let vpath = id.vpath();
        let name = if let Some(package) = id.package() {
            format!("{package}{}", vpath.as_rooted_path().display())
        } else if vpath.as_rootless_path() == FAKE_MAIN_PATH {
            FAKE_MAIN_PATH.into()
        } else if let Some(root) = self.file_store.root().as_deref() {
            vpath
                .resolve(root)
                .as_deref()
                .unwrap_or_else(|| vpath.as_rootless_path())
                .to_string_lossy()
                .into()
        } else {
            vpath.as_rootless_path().to_string_lossy().into()
        };
        Ok(name)
    }

    fn source(&'a self, id: Self::FileId) -> CodespanResult<Self::Source> {
        self.file_store
            .source(id)
            .map(|src| src.lines().clone())
            .map_err(|_| CodespanError::FileMissing)
    }

    fn line_index(&'a self, id: Self::FileId, byte_index: usize) -> CodespanResult<usize> {
        let lines = self
            .file_store
            .source(id)
            .map(|src| src.lines().clone())
            .map_err(|_| CodespanError::FileMissing)?;

        lines
            .byte_to_line(byte_index)
            .ok_or_else(|| CodespanError::IndexTooLarge {
                given: byte_index,
                max: lines.len_bytes(),
            })
    }

    fn line_range(
        &'a self,
        id: Self::FileId,
        line_index: usize,
    ) -> CodespanResult<std::ops::Range<usize>> {
        let lines = self
            .file_store
            .source(id)
            .map(|src| src.lines().clone())
            .map_err(|_| CodespanError::FileMissing)?;

        lines
            .line_to_range(line_index)
            .ok_or_else(|| CodespanError::LineTooLarge {
                given: line_index,
                max: lines.len_lines(),
            })
    }
}

fn print_diagnostics(
    world: &TypstWorld,
    errors: &[SourceDiagnostic],
    warnings: &[SourceDiagnostic],
) -> anyhow::Result<()> {
    let config = codespan_reporting::term::Config {
        tab_width: 2,
        ..Default::default()
    };

    for diagnostic in warnings.iter().chain(errors) {
        let diag = match diagnostic.severity {
            Severity::Error => Diagnostic::error(),
            Severity::Warning => Diagnostic::warning(),
        }
        .with_message(diagnostic.message.clone())
        .with_notes(
            diagnostic
                .hints
                .iter()
                .map(|e| eco_format!("hint: {e}").into())
                .collect(),
        )
        .with_labels(label(world, diagnostic.span).into_iter().collect());

        let mut writer = match diagnostic.severity {
            Severity::Error => aviutl2::logger::LockedInternalWriter::error(),
            Severity::Warning => aviutl2::logger::LockedInternalWriter::warn(),
        };
        codespan_reporting::term::emit_to_io_write(&mut writer, &config, world, &diag)?;
    }

    Ok(())
}

fn label(world: &TypstWorld, span: Span) -> Option<Label<FileId>> {
    Some(Label::primary(span.id()?, world.range(span)?))
}

pub struct RenderedImage {
    pub width: u32,
    pub height: u32,
    pub data: Vec<RgbaPixel>,
}

impl RenderedImage {
    pub fn render(page: &Page, pixel_per_pt: f32) -> Self {
        let image = typst_render::render(page, pixel_per_pt);
        let data = image
            .pixels()
            .iter()
            .map(|premultiplied| {
                let straight = premultiplied.demultiply();
                RgbaPixel {
                    r: straight.red(),
                    g: straight.green(),
                    b: straight.blue(),
                    a: straight.alpha(),
                }
            })
            .collect::<Vec<_>>();
        Self {
            width: image.width(),
            height: image.height(),
            data,
        }
    }
}
