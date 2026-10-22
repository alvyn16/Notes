fn main() {
    println!("cargo:rerun-if-changed=assets/notes.rc");
    println!("cargo:rerun-if-changed=assets/notes.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        embed_resource::compile("assets/notes.rc", embed_resource::NONE)
            .manifest_required()
            .expect("compile the Notes Windows icon resource");
    }
}
