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
