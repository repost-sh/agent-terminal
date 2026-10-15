use std::process::Command;

use crate::support::fixture;

#[test]
fn the_dashboards_width_table_is_generated_from_the_daemons_crate() {
    let generated = Command::new(fixture("width-table")).output().expect("run the width-table fixture");
    assert!(generated.status.success(), "width-table failed: {}", String::from_utf8_lossy(&generated.stderr));
    let committed = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/../dashboard/src/lib/widths.ts"))
        .expect("read dashboard/src/lib/widths.ts");
    assert!(
        generated.stdout == committed,
        "dashboard/src/lib/widths.ts is stale; from cli/, run `cargo run --quiet -p at-fixtures --bin width-table > ../dashboard/src/lib/widths.ts`"
    );
}
