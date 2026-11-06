fn main() {
    #[cfg(feature = "gui")]
    slint_build::compile_with_config(
        "ui/spike.slint",
        slint_build::CompilerConfiguration::new()
            .embed_resources(slint_build::EmbedResourcesKind::EmbedFiles),
    )
    .expect("compile qualification UI");
}
