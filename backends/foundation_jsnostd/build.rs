fn main() {
    // tell cargo to build/rebuild when any of these files changes
    println!("cargo::rerun-if-changed=src/runtime/*")
}
