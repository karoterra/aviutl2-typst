use std::{
    num::NonZeroUsize,
    sync::{LazyLock, Mutex},
};

use aviutl2::{
    AnyResult, anyhow,
    filter::{FilterConfigItemSliceExt, FilterConfigItems},
    tracing,
};
use lru::LruCache;
use typst::comemo;

use crate::typst_world::{RenderedImage, TYPST_ENGINE};

#[aviutl2::plugin(FilterPlugin)]
pub struct TypstTextPlugin {}

#[derive(aviutl2::filter::FilterConfigSelectItems)]
enum PageSizeUnit {
    #[item(name = "mm")]
    Mm,
    #[item(name = "cm")]
    Cm,
    #[item(name = "in")]
    In,
    #[item(name = "pt")]
    Pt,
    #[item(name = "px")]
    Px,
}

#[aviutl2::filter::filter_config_items]
struct TypstTextConfig {
    #[track(name = "幅", range = 1..=8192, default = 100, step = 0.1)]
    width: f64,
    #[track(name = "高さ", range = 1..=8192, default = 100, step = 0.1)]
    height: f64,
    #[select(name = "単位", items = PageSizeUnit, default = PageSizeUnit::Px)]
    unit: PageSizeUnit,
    #[track(name = "スケール", range = 1..=10, default = 1, step = 0.01)]
    ppt: f64,
    #[text(name = "テキスト")]
    text: String,
}

pub type TextRenderCacheKey = i64; // ObjectInfo.effect_id
pub struct TextRenderCacheValue {
    ppt: f64,
    text: String,
    image: RenderedImage,
}

pub static TEXT_RENDER_CACHE: LazyLock<Mutex<LruCache<TextRenderCacheKey, TextRenderCacheValue>>> =
    LazyLock::new(|| Mutex::new(LruCache::new(NonZeroUsize::new(4).unwrap())));

impl aviutl2::filter::FilterPlugin for TypstTextPlugin {
    type Userdata = ();

    fn new(_info: aviutl2::common::AviUtl2Info) -> AnyResult<Self> {
        Ok(Self {})
    }

    fn plugin_info(&self) -> aviutl2::filter::FilterPluginTable {
        aviutl2::filter::FilterPluginTable {
            name: "Typstテキスト".to_string(),
            label: Some("Typst".to_string()),
            information: format!(
                "Typst Text object v{} by karoterra",
                env!("CARGO_PKG_VERSION")
            ),
            flags: aviutl2::bitflag!(aviutl2::filter::FilterPluginFlags {
                video: true,
                input: true,
            }),
            config_items: TypstTextConfig::to_config_items(),
        }
    }

    fn proc_video(
        &self,
        config: &[aviutl2::filter::FilterConfigItem],
        video: &mut aviutl2::filter::FilterProcVideo<Self::Userdata>,
    ) -> AnyResult<()> {
        let config = config.to_struct::<TypstTextConfig>();

        let header = match config.unit {
            PageSizeUnit::Mm => format!(
                "#set page(width: {}mm, height: {}mm)",
                config.width, config.height
            ),
            PageSizeUnit::Cm => format!(
                "#set page(width: {}cm, height: {}cm)",
                config.width, config.height
            ),
            PageSizeUnit::In => format!(
                "#set page(width: {}in, height: {}in)",
                config.width, config.height
            ),
            PageSizeUnit::Pt => format!(
                "#set page(width: {}pt, height: {}pt)",
                config.width, config.height
            ),
            PageSizeUnit::Px => format!(
                "#set page(width: {}pt, height: {}pt)",
                config.width / config.ppt,
                config.height / config.ppt
            ),
        };

        let text = format!("{}\n{}", header, config.text);

        let mut cache = TEXT_RENDER_CACHE.lock().unwrap();
        let key = video.object.effect_id;
        let value = cache.try_get_or_insert_mut(key, || compile_and_render(&text, config.ppt))?;
        if value.ppt != config.ppt || value.text != text {
            *value = compile_and_render(&text, config.ppt)?;
        }

        tracing::debug!(
            "Rendered image: {} x {}",
            value.image.width,
            value.image.height
        );
        video.set_image_data(&value.image.data, value.image.width, value.image.height);

        comemo::evict(0);

        Ok(())
    }
}

fn compile_and_render(text: &str, pixel_per_pt: f64) -> AnyResult<TextRenderCacheValue> {
    let engine = TYPST_ENGINE.read().unwrap();
    let doc = engine.compile_text(text)?;
    if doc.pages().is_empty() {
        comemo::evict(0);
        anyhow::bail!("Compiled Typst document has no pages");
    }

    let page = doc.pages().first().unwrap();
    let image = RenderedImage::render(page, pixel_per_pt);
    Ok(TextRenderCacheValue {
        ppt: pixel_per_pt,
        text: text.to_string(),
        image,
    })
}
