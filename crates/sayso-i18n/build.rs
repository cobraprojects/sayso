// The catalogs are compiled into the crate, so a change to any of them must
// build it again. Cargo scans a directory given here for changed files.
fn main() {
    println!("cargo:rerun-if-changed=locales");
}
