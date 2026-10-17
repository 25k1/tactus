fn main() {
    println!("cargo::rerun-if-changed=assets/windows");
    embed_resource::compile("assets/windows/tactus.rc", embed_resource::NONE).manifest_required().unwrap();
}
