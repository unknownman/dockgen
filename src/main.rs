mod analyzer;
mod cli;
mod detector;
mod models;
mod templates;

fn main() {
    println!("dockgen v{}", env!("CARGO_PKG_VERSION"));
}
