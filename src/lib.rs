mod typst_text_plugin;
mod typst_world;

use aviutl2::{AnyResult, anyhow::anyhow, tracing};

use crate::typst_text_plugin::TypstTextPlugin;
use crate::typst_world::{TYPST_ENGINE, TypstEngine};

#[aviutl2::plugin(GenericPlugin)]
struct TypstPlugin {
    text_plugin: aviutl2::generic::SubPlugin<TypstTextPlugin>,
    _engine: &'static TypstEngine,
}

impl aviutl2::generic::GenericPlugin for TypstPlugin {
    fn new(info: aviutl2::common::AviUtl2Info) -> AnyResult<Self> {
        Self::init_logging();
        let engine = TYPST_ENGINE
            .as_ref()
            .ok_or_else(|| anyhow!("Failed to initialize TypstEngine"))?;
        Ok(Self {
            text_plugin: aviutl2::generic::SubPlugin::new_filter_plugin(&info)?,
            _engine: engine,
        })
    }

    fn plugin_info(&self) -> aviutl2::generic::GenericPluginTable {
        aviutl2::generic::GenericPluginTable {
            name: "Typst for AviUtl2".to_string(),
            information: format!(
                "Typst for AviUtl2 v{} by karoterra",
                env!("CARGO_PKG_VERSION")
            ),
        }
    }

    fn register(&mut self, registry: &mut aviutl2::generic::HostAppHandle) {
        registry.register_filter_plugin(&self.text_plugin);
    }
}

impl TypstPlugin {
    fn init_logging() {
        aviutl2::tracing_subscriber::fmt()
            .with_max_level(if cfg!(debug_assertions) {
                tracing::Level::DEBUG
            } else {
                tracing::Level::INFO
            })
            .event_format(aviutl2::logger::AviUtl2Formatter)
            .with_writer(aviutl2::logger::AviUtl2LogWriter)
            .init();
    }
}

aviutl2::register_generic_plugin!(TypstPlugin);
