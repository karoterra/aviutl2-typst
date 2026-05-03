mod path;
mod typst_file;
mod typst_file_plugin;
mod typst_package;
mod typst_text_plugin;
mod typst_world;

use std::path::Path;

use aviutl2::{AnyResult, anyhow, tracing};
use typst::comemo;

use crate::path::DLL_DIR;
use crate::typst_file_plugin::{FILE_PLUGIN_CACHE, TypstFilePlugin};
use crate::typst_text_plugin::{TEXT_RENDER_CACHE, TypstTextPlugin};
use crate::typst_world::TYPST_ENGINE;

static PLUGIN_NAME: &str = "Typst for AviUtl2";
static EDIT_HANDLE: aviutl2::generic::GlobalEditHandle = aviutl2::generic::GlobalEditHandle::new();

#[aviutl2::plugin(GenericPlugin)]
struct TypstPlugin {
    text_plugin: aviutl2::generic::SubPlugin<TypstTextPlugin>,
    file_plugin: aviutl2::generic::SubPlugin<TypstFilePlugin>,
}

impl aviutl2::generic::GenericPlugin for TypstPlugin {
    fn new(info: aviutl2::common::AviUtl2Info) -> AnyResult<Self> {
        Self::init_logging();

        if DLL_DIR.is_none() {
            return Err(anyhow::anyhow!("Failed to determine DLL directory."));
        }
        tracing::debug!("DLL directory: {:?}", DLL_DIR.as_ref().unwrap());

        Ok(Self {
            text_plugin: aviutl2::generic::SubPlugin::new_filter_plugin(&info)?,
            file_plugin: aviutl2::generic::SubPlugin::new_filter_plugin(&info)?,
        })
    }

    fn plugin_info(&self) -> aviutl2::generic::GenericPluginTable {
        aviutl2::generic::GenericPluginTable {
            name: PLUGIN_NAME.to_string(),
            information: format!(
                "{} v{} by karoterra",
                PLUGIN_NAME,
                env!("CARGO_PKG_VERSION")
            ),
        }
    }

    fn register(&mut self, registry: &mut aviutl2::generic::HostAppHandle) {
        EDIT_HANDLE.init(registry.create_edit_handle());

        registry.register_filter_plugin(&self.text_plugin);
        registry.register_filter_plugin(&self.file_plugin);

        let filters = aviutl2::file_filters! {
            "Typst file" => ["typ"]
        };
        registry.register_file_drop_handler(PLUGIN_NAME, &filters, |path| {
            let res = EDIT_HANDLE
                .call_edit_section(|edit_section| {
                    let position = edit_section.get_mouse_layer_frame()?.unwrap_or(
                        aviutl2::generic::LayerFrameData {
                            layer: edit_section.info.layer,
                            frame: edit_section.info.frame,
                        },
                    );

                    let mut object_alias = aviutl2::alias::Table::new();
                    let mut object = aviutl2::alias::Table::new();
                    object.insert_value(
                        "name",
                        path.file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("Typstファイル"),
                    );
                    let mut object_0 = aviutl2::alias::Table::new();
                    object_0.insert_value("effect.name", "Typstファイル");
                    object_0.insert_value(
                        "ファイル",
                        path.to_str().ok_or(anyhow::anyhow!(
                            "Failed to convert path to string: {path:?}"
                        ))?,
                    );
                    object.insert_table("0", object_0);
                    object_alias.insert_table("Object", object);

                    edit_section.create_object_from_alias(
                        &object_alias.to_string(),
                        position.layer,
                        position.frame,
                        0,
                    )?;

                    anyhow::Ok(())
                })
                .map_err(anyhow::Error::from)
                .flatten();
            if let Err(e) = res {
                tracing::error!("Failed to handle file drop: {e}");
            }
        });
    }

    fn on_project_load(&mut self, _project: &mut aviutl2::generic::ProjectFile) {
        self.update_project_dir(_project.get_path().as_deref());
    }

    fn on_project_save(&mut self, _project: &mut aviutl2::generic::ProjectFile) {
        self.update_project_dir(_project.get_path().as_deref());
    }

    fn on_clear_cache(&mut self, _edit_section: &aviutl2::generic::EditSection) {
        tracing::debug!("Delete typst cache");
        TEXT_RENDER_CACHE.lock().unwrap().clear();
        FILE_PLUGIN_CACHE.lock().unwrap().clear();
        comemo::evict(0);
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
