//! Native check that `tests/golden.rs` lists every generator golden file, so a new file cannot
//! be left out of the wasm parity test.

#![cfg(not(target_arch = "wasm32"))]

use std::path::Path;

#[test]
fn wasm_test_lists_every_generator_file() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let listing = std::fs::read_to_string(root.join("tests/golden.rs")).unwrap();
    let mut n = 0;
    for dir in ["uniform_water_sort", "turan_water_sort"] {
        for entry in std::fs::read_dir(root.join("..").join(dir).join("tests/golden")).unwrap() {
            let name = entry.unwrap().file_name().into_string().unwrap();
            let line = format!("\"../../{dir}/tests/golden/{name}\"");
            assert!(
                listing.contains(&line),
                "tests/golden.rs does not list {dir}/{name}"
            );
            n += 1;
        }
    }
    let listed = listing
        .matches("\"../../uniform_water_sort/tests/golden/")
        .count()
        + listing
            .matches("\"../../turan_water_sort/tests/golden/")
            .count();
    assert_eq!(listed, n, "tests/golden.rs lists files that do not exist");
}
