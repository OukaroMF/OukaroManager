#[path = "../../shared/android_install_path.rs"]
mod android_install_path;
#[path = "../../shared/android_package.rs"]
mod android_package;
#[path = "../../shared/android_package_state.rs"]
mod android_package_state;
#[path = "../../shared/android_xml.rs"]
mod android_xml;
mod cli;
mod config;
mod defs;

fn main() {
    cli::run().unwrap_or_else(|e| {
        for c in e.chain() {
            eprintln!("{c:#?}");
        }
        eprintln!("{:#?}", e.backtrace());
    });
}
