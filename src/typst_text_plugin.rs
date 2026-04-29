use aviutl2::{
    AnyResult,
    anyhow::anyhow,
    filter::{FilterConfigItemSliceExt, FilterConfigItems},
    tracing,
};

use crate::typst_world::{TYPST_ENGINE, TypstEngine};

#[aviutl2::plugin(FilterPlugin)]
pub struct TypstTextPlugin {
    engine: &'static TypstEngine,
}

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
    ppt: f32,
    #[text(name = "テキスト")]
    text: String,
}

impl aviutl2::filter::FilterPlugin for TypstTextPlugin {
    fn new(_info: aviutl2::common::AviUtl2Info) -> AnyResult<Self> {
        let engine = TYPST_ENGINE
            .as_ref()
            .ok_or_else(|| anyhow!("Failed to initialize TypstEngine"))?;
        Ok(Self { engine })
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
                as_object: true,
            }),
            config_items: TypstTextConfig::to_config_items(),
        }
    }

    fn proc_video(
        &self,
        config: &[aviutl2::filter::FilterConfigItem],
        video: &mut aviutl2::filter::FilterProcVideo,
    ) -> AnyResult<()> {
        let config = config.to_struct::<TypstTextConfig>();

        let ppt = config.ppt as f64;
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
                config.width / ppt,
                config.height / ppt
            ),
        };

        let text = format!("{}\n{}", header, config.text);

        let image = self.engine.compile(&text, config.ppt)?;
        tracing::debug!("Rendered image: {} x {}", image.width, image.height);
        video.set_image_data(&image.data, image.width, image.height);

        Ok(())
    }
}
