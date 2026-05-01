use aviutl2::tracing;
use typst::syntax::package::PackageSpec;
use typst_kit::download::{DownloadState, Downloader, Progress};
use typst_kit::package::PackageStorage;

use crate::path::{get_package_cache_dir, get_package_dir};

fn new_downloader() -> Downloader {
    let user_agent = concat!("typst_kr/", env!("CARGO_PKG_VERSION"));
    Downloader::new(user_agent)
}

pub fn new_storage() -> PackageStorage {
    PackageStorage::new(get_package_cache_dir(), get_package_dir(), new_downloader())
}

pub struct PackageDownloadProgress<'a> {
    pub package: &'a PackageSpec,
}

impl Progress for PackageDownloadProgress<'_> {
    fn print_start(&mut self) {
        tracing::debug!("Start downloading package: {}", self.package);
    }

    fn print_progress(&mut self, _state: &DownloadState) {}

    fn print_finish(&mut self, state: &DownloadState) {
        tracing::debug!(
            "Finished downloading package: {}, {}",
            self.package,
            as_bytes_unit(state.total_downloaded)
        );
    }
}

fn as_bytes_unit(size: usize) -> String {
    const KI: f64 = 1024.0;
    const MI: f64 = KI * KI;
    const GI: f64 = KI * KI * KI;

    let size = size as f64;

    if size >= GI {
        format!("{:.1} GiB", size / GI)
    } else if size >= MI {
        format!("{:.1} MiB", size / MI)
    } else if size >= KI {
        format!("{:.1} KiB", size / KI)
    } else {
        format!("{size:3} B")
    }
}
