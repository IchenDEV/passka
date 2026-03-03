fn main() {
    let out_dir = "generated";
    swift_bridge_build::parse_bridges(vec!["src/lib.rs"])
        .write_all_concatenated(out_dir, env!("CARGO_PKG_NAME"));
}
