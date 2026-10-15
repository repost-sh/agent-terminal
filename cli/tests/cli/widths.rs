use std::path::Path;
use std::process::Command;

use crate::support::fixture;

const TABLES: [(&str, &str); 3] = [("version", "version.ts"), ("zero", "zero.ts"), ("wide", "wide.ts")];

#[test]
fn the_dashboards_width_table_is_generated_from_the_daemons_crate() {
    for (table, file) in TABLES {
        the_committed_table_is_what_the_generator_prints(table, file);
    }
}

fn the_committed_table_is_what_the_generator_prints(table: &str, file: &str) {
    let generated = Command::new(fixture("width-table")).arg(table).output().expect("run the width-table fixture");
    assert!(generated.status.success(), "width-table failed: {}", String::from_utf8_lossy(&generated.stderr));
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../dashboard/src/lib/widths").join(file);
    let committed = std::fs::read(&path).unwrap_or_else(|e| panic!("read dashboard/src/lib/widths/{file}: {e}"));
    assert!(
        generated.stdout == committed,
        "dashboard/src/lib/widths/{file} is stale; from cli/, run `cargo run --quiet -p at-fixtures --bin width-table {table} > ../dashboard/src/lib/widths/{file}`"
    );
}
