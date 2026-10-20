fn main() {
    if std::env::args().nth(1).as_deref() == Some("--version") {
        println!("x9s {}", env!("CARGO_PKG_VERSION"));
        return;
    }

    println!("x9s starter scaffold — see docs/roadmap.md for Phase 1 implementation");
}
