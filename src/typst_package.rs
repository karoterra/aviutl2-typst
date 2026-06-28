use aviutl2::tracing;
use typst::ecow::{EcoString, eco_format};
use typst::syntax::package::PackageSpec;
use typst_kit::downloader::{
    Downloader, Progress, ProgressDownloader, ProgressReporter, SystemDownloader,
};
use typst_kit::packages::{FsPackages, SystemPackages, UniversePackages};

use crate::path::{get_package_cache_dir, get_package_dir};

fn new_downloader() -> impl Downloader {
    let user_agent = concat!("typst_kr/", env!("CARGO_PKG_VERSION"));
    let system = SystemDownloader::new(user_agent);

    ProgressDownloader::new(system, |key| {
        let name = if let Some(spec) = key.downcast_ref::<PackageSpec>() {
            Some(eco_format!("{spec}"))
        } else if let Some(&s @ "release") = key.downcast_ref::<&str>() {
            Some(s.into())
        } else {
            None
        };
        PrintProgress { name }
    })
}

struct PrintProgress {
    name: Option<EcoString>,
}

impl ProgressReporter for PrintProgress {
    fn start(&mut self, _progress: &Progress) {
        tracing::debug!(
            "Start downloading package: {}",
            self.name.as_deref().unwrap_or("<unknown>")
        );
    }

    fn update(&mut self, _progress: &Progress) {}

    fn finish(&mut self, progress: &Progress) {
        tracing::debug!(
            "Finished downloading package: {}, {}",
            self.name.as_deref().unwrap_or("<unknown>"),
            progress
        );
    }
}

pub fn new_packages() -> SystemPackages {
    SystemPackages::from_parts(
        get_package_dir().map(FsPackages::new),
        get_package_cache_dir().map(FsPackages::new),
        UniversePackages::new(new_downloader()),
    )
}
