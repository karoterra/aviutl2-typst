use std::path::PathBuf;

use aviutl2::{
    AnyResult, anyhow,
    filter::{FilterConfigItemSliceExt, FilterConfigItems},
    tracing,
};
use typst::comemo;

use crate::typst_world::{RenderedImage, TYPST_ENGINE};

#[aviutl2::plugin(FilterPlugin)]
pub struct TypstFilePlugin {}

#[aviutl2::filter::filter_config_items]
struct TypstFileConfig {
    #[track(name = "スケール", range = 1..=10, default = 1, step = 0.01)]
    ppt: f32,
    #[track(name = "ページ", range = 1..=100, default = 1, step = 1.0)]
    page: usize,
    #[file(name = "ファイル", filters = {"Typst file" => ["typ"]})]
    file: Option<PathBuf>,
}

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
                as_object: true
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
        match config.file {
            None => anyhow::bail!("Typst file is not set"),
            Some(path) => {
                let engine = TYPST_ENGINE.read().unwrap();
                let doc = engine.compile_file(&path)?;
                if doc.pages.is_empty() {
                    comemo::evict(0);
                    anyhow::bail!("Compiled Typst document has no pages");
                }

                let idx = std::cmp::min(config.page - 1, doc.pages.len() - 1);
                let page = &doc.pages[idx];

                let image = RenderedImage::render(page, config.ppt);
                tracing::debug!("Rendered image: {} x {}", image.width, image.height);
                video.set_image_data(&image.data, image.width, image.height);
            }
        };

        comemo::evict(0);

        Ok(())
    }
}
