fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    cc::Build::new()
        .cpp(true)
        .file("native/plugin.cpp")
        .include("include")
        .flag_if_supported("/std:c++17")
        .compile("trafficmonitor_plugin");

    println!("cargo:rerun-if-changed=native/plugin.cpp");
    println!("cargo:rerun-if-changed=include/PluginInterface.h");
}
