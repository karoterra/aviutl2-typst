use aviutl2::AnyResult;

#[aviutl2::plugin(GenericPlugin)]
struct TypstPlugin {}

impl aviutl2::generic::GenericPlugin for TypstPlugin {
    fn new(_info: aviutl2::common::AviUtl2Info) -> AnyResult<Self> {
        Ok(Self {})
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

    fn register(&mut self, _registry: &mut aviutl2::generic::HostAppHandle) {}
}

aviutl2::register_generic_plugin!(TypstPlugin);
