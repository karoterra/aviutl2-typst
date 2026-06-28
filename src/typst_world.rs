use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex, RwLock};

use aviutl2::{anyhow, filter::RgbaPixel, tracing};
use codespan_reporting::diagnostic::{Diagnostic, Label};
use typst::{
    Library, LibraryExt, World, WorldExt,
    diag::{FileResult, Severity, SourceDiagnostic, Warned},
    foundations::{Bytes, Datetime, Duration},
    syntax::{FileId, Lines, Source, VirtualRoot},
    text::{Font, FontBook},
    utils::{LazyHash, Scalar},
};
use typst_kit::datetime::Time;
use typst_kit::files::FileStore;
use typst_kit::fonts::FontStore;
use typst_kit::packages::SystemPackages;
use typst_layout::{Page, PagedDocument};
use typst_render::RenderOptions;

use crate::typst_file::{FAKE_MAIN_ID, TypstFileLoader};
use crate::typst_package::new_packages;

type CodespanError = codespan_reporting::files::Error;
type CodespanResult<T> = Result<T, CodespanError>;

pub static TYPST_ENGINE: LazyLock<RwLock<TypstEngine>> =
    LazyLock::new(|| RwLock::new(TypstEngine::new()));

pub struct TypstEngine {
    library: LazyHash<Library>,
    fonts: FontStore,
    project_dir: Option<PathBuf>,
    packages: Arc<Mutex<SystemPackages>>,
}

impl TypstEngine {
    fn new() -> Self {
        let library = Library::default();

        let mut fonts = FontStore::new();
        fonts.extend(typst_kit::fonts::system());
        fonts.extend(typst_kit::fonts::embedded());

        let packages = Arc::new(Mutex::new(new_packages()));

        Self {
            library: LazyHash::new(library),
            fonts,
            project_dir: None,
            packages,
        }
    }

    pub fn set_project_dir(&mut self, dir: Option<PathBuf>) {
        self.project_dir = dir;
    }

    pub fn compile_text(&self, source: &str) -> anyhow::Result<PagedDocument> {
        let loader =
            TypstFileLoader::from_text(source, self.project_dir.clone(), self.packages.clone());
        let world = self.create_world(loader);
        self.compile(&world)
    }

    pub fn compile_file(&self, path: &Path) -> anyhow::Result<PagedDocument> {
        let loader = TypstFileLoader::from_file(path, self.packages.clone())?;
        let world = self.create_world(loader);
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

    fn create_world(&self, loader: TypstFileLoader) -> TypstWorld<'_> {
        TypstWorld {
            library: &self.library,
            fonts: &self.fonts,
            now: Time::system(),
            files: FileStore::new(loader),
        }
    }
}

struct TypstWorld<'a> {
    library: &'a LazyHash<Library>,
    fonts: &'a FontStore,
    now: Time,
    files: FileStore<TypstFileLoader>,
}

impl World for TypstWorld<'_> {
    fn library(&self) -> &LazyHash<Library> {
        self.library
    }

    fn book(&self) -> &LazyHash<FontBook> {
        self.fonts.book()
    }

    fn main(&self) -> FileId {
        self.files.loader().main()
    }

    fn source(&self, id: FileId) -> FileResult<Source> {
        tracing::debug!("Loading source: {:?}", id);
        self.files.source(id)
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        tracing::debug!("Loading file: {:?}", id);
        self.files.file(id)
    }

    fn font(&self, index: usize) -> Option<Font> {
        self.fonts.font(index)
    }

    fn today(&self, offset: Option<Duration>) -> Option<Datetime> {
        self.now.today(offset)
    }
}

impl<'a> codespan_reporting::files::Files<'a> for TypstWorld<'_> {
    type FileId = FileId;
    type Name = String;
    type Source = Lines<String>;

    fn name(&'a self, id: Self::FileId) -> CodespanResult<Self::Name> {
        let vpath = id.vpath();
        let name = match id.root() {
            VirtualRoot::Project => {
                if id == *FAKE_MAIN_ID {
                    "<text>".to_string()
                } else if let Some(root) = self.files.loader().project() {
                    vpath
                        .realize(root.path())
                        .ok()
                        .map(|path| path.to_string_lossy().into_owned())
                        .unwrap_or_else(|| vpath.get_without_slash().to_string())
                } else {
                    vpath.get_without_slash().to_string()
                }
            }
            VirtualRoot::Package(package) => {
                format!("{package}{}", vpath.get_with_slash())
            }
        };
        Ok(name)
    }

    fn source(&'a self, id: Self::FileId) -> CodespanResult<Self::Source> {
        self.files
            .source(id)
            .map(|src| src.lines().clone())
            .map_err(|_| CodespanError::FileMissing)
    }

    fn line_index(&'a self, id: Self::FileId, byte_index: usize) -> CodespanResult<usize> {
        let lines = self
            .files
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
            .files
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
                .filter(|s| s.span.is_detached())
                .map(|s| format!("hint: {}", s.v))
                .collect(),
        )
        .with_labels(
            diagnostic
                .span
                .id()
                .and_then(|id| {
                    let range = world.range(diagnostic.span)?;
                    Some(Label::primary(id, range))
                })
                .into_iter()
                .chain(diagnostic.hints.iter().filter_map(|hint| {
                    let id = hint.span.id()?;
                    let range = world.range(hint.span)?;
                    Some(Label::secondary(id, range).with_message(&hint.v))
                }))
                .collect(),
        );

        let mut writer = match diagnostic.severity {
            Severity::Error => aviutl2::logger::LockedInternalWriter::error(),
            Severity::Warning => aviutl2::logger::LockedInternalWriter::warn(),
        };
        codespan_reporting::term::emit_to_io_write(&mut writer, &config, world, &diag)?;
    }

    Ok(())
}

pub struct RenderedImage {
    pub width: u32,
    pub height: u32,
    pub data: Vec<RgbaPixel>,
}

impl RenderedImage {
    pub fn render(page: &Page, pixel_per_pt: f64) -> Self {
        let opts = RenderOptions {
            pixel_per_pt: Scalar::new(pixel_per_pt),
            render_bleed: false,
        };
        let image = typst_render::render(page, &opts);
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
