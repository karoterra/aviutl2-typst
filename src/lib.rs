mod typst_text_plugin;
mod typst_world;

use std::path::Path;

use aviutl2::{AnyResult, tracing};

use crate::typst_text_plugin::TypstTextPlugin;
use crate::typst_world::TYPST_ENGINE;

#[aviutl2::plugin(GenericPlugin)]
struct TypstPlugin {
    text_plugin: aviutl2::generic::SubPlugin<TypstTextPlugin>,
}

impl aviutl2::generic::GenericPlugin for TypstPlugin {
    fn new(info: aviutl2::common::AviUtl2Info) -> AnyResult<Self> {
        Self::init_logging();
        Ok(Self {
            text_plugin: aviutl2::generic::SubPlugin::new_filter_plugin(&info)?,
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

    fn on_project_load(&mut self, _project: &mut aviutl2::generic::ProjectFile) {
        self.update_project_dir(_project.get_path().as_deref());
    }

    fn on_project_save(&mut self, _project: &mut aviutl2::generic::ProjectFile) {
        self.update_project_dir(_project.get_path().as_deref());
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

    fn update_project_dir(&mut self, project_path: Option<&Path>) {
        let mut engine = TYPST_ENGINE.write().unwrap();
        match project_path {
            Some(path) => {
                if path.as_os_str().is_empty() {
                    engine.set_project_dir(None);
                } else {
                    let project_dir = path.parent().map(|p| p.to_path_buf());
                    engine.set_project_dir(project_dir);
                }
            }
            None => {
                engine.set_project_dir(None);
            }
        }
    }
}

aviutl2::register_generic_plugin!(TypstPlugin);
