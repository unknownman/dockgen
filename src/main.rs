mod analyzer;
mod cli;
mod detector;
mod generator;
mod models;
mod templates;

fn main() {
    println!("dockgen v{}", env!("CARGO_PKG_VERSION"));
}
