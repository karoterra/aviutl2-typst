use std::{
    num::NonZeroUsize,
    path::{Path, PathBuf},
    sync::{LazyLock, Mutex},
};

use aviutl2::{
    AnyResult, anyhow,
    filter::{FilterConfigItemSliceExt, FilterConfigItems},
    tracing,
};
use lru::LruCache;
use typst::comemo;
use typst_layout::Page;

use crate::typst_world::{RenderedImage, TYPST_ENGINE};

#[aviutl2::plugin(FilterPlugin)]
pub struct TypstFilePlugin {}

#[aviutl2::filter::filter_config_items]
struct TypstFileConfig {
    #[track(name = "スケール", range = 1..=10, default = 1, step = 0.01)]
    ppt: f64,
    #[track(name = "ページ", range = 1..=100, default = 1, step = 1.0)]
    page: usize,
    #[file(name = "ファイル", filters = {"Typst file" => ["typ"]})]
    file: Option<PathBuf>,
}

pub type FileCompileCacheKey = PathBuf;
pub type FileCompileCacheValue = Vec<Page>;

pub type FileRenderCacheKey = i64; // ObjectInfo.effect_id
pub struct FileRenderCacheValue {
    ppt: f64,
    page: usize,
    path: PathBuf,
    image: RenderedImage,
}

pub struct FilePluginCache {
    pub compile: LruCache<FileCompileCacheKey, FileCompileCacheValue>,
    pub render: LruCache<FileRenderCacheKey, FileRenderCacheValue>,
}

impl FilePluginCache {
    pub fn clear(&mut self) {
        self.compile.clear();
        self.render.clear();
    }
}

pub static FILE_PLUGIN_CACHE: LazyLock<Mutex<FilePluginCache>> = LazyLock::new(|| {
    Mutex::new(FilePluginCache {
        compile: LruCache::new(NonZeroUsize::new(4).unwrap()),
        render: LruCache::new(NonZeroUsize::new(4).unwrap()),
    })
});

impl aviutl2::filter::FilterPlugin for TypstFilePlugin {
    fn new(_info: aviutl2::common::AviUtl2Info) -> AnyResult<Self> {
        Ok(Self {})
    }

    fn plugin_info(&self) -> aviutl2::filter::FilterPluginTable {
        aviutl2::filter::FilterPluginTable {
            name: "Typstファイル".to_string(),
            label: Some("Typst".to_string()),
            information: format!(
                "Typst File object v{} by karoterra",
                env!("CARGO_PKG_VERSION")
            ),
            flags: aviutl2::bitflag!(aviutl2::filter::FilterPluginFlags {
                video: true,
                input: true,
            }),
            config_items: TypstFileConfig::to_config_items(),
        }
    }

    fn proc_video(
        &self,
        config: &[aviutl2::filter::FilterConfigItem],
        video: &mut aviutl2::filter::FilterProcVideo,
    ) -> AnyResult<()> {
        let config = config.to_struct::<TypstFileConfig>();
        match config.file.filter(|p| !p.as_os_str().is_empty()) {
            None => anyhow::bail!("Typst file is not set"),
            Some(path) => {
                let mut cache = FILE_PLUGIN_CACHE.lock().unwrap();
                let FilePluginCache {
                    compile: compile_cache,
                    render: render_cache,
                } = &mut *cache;

                let key = path.clone();
                let pages = compile_cache.try_get_or_insert(key, || compile(&path))?;

                let render_key = video.object.effect_id;
                let render_value = render_cache.try_get_or_insert_mut(render_key, || {
                    render(pages, config.ppt, config.page, &path)
                })?;

                if render_value.ppt != config.ppt
                    || render_value.page != config.page
                    || render_value.path != path
                {
                    *render_value = render(pages, config.ppt, config.page, &path)?;
                }

                tracing::debug!(
                    "Rendered image: {} x {}",
                    render_value.image.width,
                    render_value.image.height
                );
                video.set_image_data(
                    &render_value.image.data,
                    render_value.image.width,
                    render_value.image.height,
                );
            }
        };

        comemo::evict(0);

        Ok(())
    }
}

fn compile(path: &Path) -> AnyResult<FileCompileCacheValue> {
    let engine = TYPST_ENGINE.read().unwrap();
    let doc = engine.compile_file(path)?;
    if doc.pages().is_empty() {
        comemo::evict(0);
        anyhow::bail!("Compiled Typst document has no pages");
    }

    Ok(doc.pages().to_vec())
}

fn render(pages: &[Page], ppt: f64, page: usize, path: &Path) -> AnyResult<FileRenderCacheValue> {
    let idx = std::cmp::min(page - 1, pages.len() - 1);
    let image = RenderedImage::render(&pages[idx], ppt);

    Ok(FileRenderCacheValue {
        ppt,
        page,
        path: path.into(),
        image,
    })
}
