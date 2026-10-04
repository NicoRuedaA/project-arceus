//! Preflight and emitter tests for the sheet book machinery.

use std::fs;
use std::path::{Path, PathBuf};

fn tmp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "sheetty-test-{}-{}-{:?}",
        name,
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn write(dir: &Path, name: &str, content: &str) {
    fs::write(dir.join(name), content).unwrap();
}

fn errors(findings: &[sheetty::Finding]) -> Vec<&sheetty::Finding> {
    findings
        .iter()
        .filter(|f| f.severity == sheetty::Severity::Error)
        .collect()
}

const GOOD: &str = "# sheet: 02-plan\n# version: 1\n# generator: test\n# target: test\nid:string*\tkind:enum\tstatus:enum\nfoo\ttask\ttodo\n";

#[test]
fn good_sheet_passes() {
    let dir = tmp("good");
    write(&dir, "02-plan.tsv", GOOD);
    let sheets = sheetty::load_dir(&dir).unwrap();
    let findings = sheetty::check(&sheets);
    assert!(errors(&findings).is_empty(), "{:?}", errors(&findings));
}

#[test]
fn bom_is_rejected() {
    let dir = tmp("bom");
    fs::write(dir.join("02-plan.tsv"), format!("\u{feff}{GOOD}")).unwrap();
    assert!(sheetty::load_dir(&dir).is_err());
}

#[test]
fn crlf_is_rejected() {
    let dir = tmp("crlf");
    write(&dir, "02-plan.tsv", &GOOD.replace('\n', "\r\n"));
    assert!(sheetty::load_dir(&dir).is_err());
}

#[test]
fn duplicate_id_is_error() {
    let dir = tmp("dup");
    write(
        &dir,
        "02-plan.tsv",
        "# sheet: 02-plan\n# version: 1\n# generator: test\n# target: test\nid:string*\tkind:enum\tstatus:enum\nfoo\ttask\ttodo\nfoo\ttask\ttodo\n",
    );
    let sheets = sheetty::load_dir(&dir).unwrap();
    let findings = sheetty::check(&sheets);
    assert!(findings.iter().any(|f| f.code == "E-L0-DUPKEY"));
}

#[test]
fn bad_id_charset_is_error() {
    let dir = tmp("charset");
    write(
        &dir,
        "02-plan.tsv",
        "# sheet: 02-plan\n# version: 1\n# generator: test\n# target: test\nid:string*\tkind:enum\tstatus:enum\nBad-Id\ttask\ttodo\n",
    );
    let sheets = sheetty::load_dir(&dir).unwrap();
    let findings = sheetty::check(&sheets);
    assert!(findings.iter().any(|f| f.code == "E-L0-KEYCHARS"));
}

#[test]
fn missing_manifest_key_is_error() {
    let dir = tmp("manifest");
    write(
        &dir,
        "02-plan.tsv",
        "# sheet: 02-plan\n# version: 1\nid:string*\tkind:enum\tstatus:enum\nfoo\ttask\ttodo\n",
    );
    let sheets = sheetty::load_dir(&dir).unwrap();
    let findings = sheetty::check(&sheets);
    assert!(findings.iter().any(|f| f.code == "E-L0-MANIFEST"));
}

#[test]
fn unresolved_reference_is_error() {
    let dir = tmp("refs");
    write(&dir, "02-plan.tsv", GOOD);
    write(
        &dir,
        "03-impl.tsv",
        "# sheet: 03-impl\n# version: 1\n# generator: test\n# target: test\nid:string*\tref:string -> 02-plan.id\tstatus:enum\nfoo\tfoo\tdone\nbar\tbar\tdone\n",
    );
    let sheets = sheetty::load_dir(&dir).unwrap();
    let findings = sheetty::check(&sheets);
    assert!(findings
        .iter()
        .any(|f| f.code == "E-L2-REF" && f.message.contains("bar")));
}

#[test]
fn coverage_gaps_are_warnings() {
    let dir = tmp("coverage");
    write(&dir, "02-plan.tsv", GOOD);
    write(
        &dir,
        "03-impl.tsv",
        "# sheet: 03-impl\n# version: 1\n# generator: test\n# target: test\nid:string*\tstatus:enum\nother\tdone\n",
    );
    let sheets = sheetty::load_dir(&dir).unwrap();
    let findings = sheetty::check(&sheets);
    assert!(errors(&findings).is_empty());
    assert!(findings.iter().any(|f| f.code == "W-L3-UNIMPLEMENTED"));
    assert!(findings.iter().any(|f| f.code == "W-L3-ORPHAN"));
}

#[test]
fn emitter_is_deterministic() {
    let sheets_dir = tmp("emit-sheets");
    write(&sheets_dir, "02-plan.tsv", GOOD);
    write(
        &sheets_dir,
        "01-schema.tsv",
        "# sheet: 01-schema\n# version: 1\n# generator: test\n# target: test\nsheet:string\tcolumn:string\ttype:string\n02-plan\tid\tstring\n",
    );
    let out1 = tmp("emit-out1");
    let out2 = tmp("emit-out2");
    for out in [&out1, &out2] {
        sheetty::run(sheetty::Config {
            sheets_dir: sheets_dir.clone(),
            out_dir: out.clone(),
            fail_on_preflight_error: true,
            emit_docs: false,
        })
        .unwrap();
    }
    let a = fs::read_to_string(out1.join("sheets/plan.rs")).unwrap();
    let b = fs::read_to_string(out2.join("sheets/plan.rs")).unwrap();
    assert_eq!(a, b);
    assert!(a.contains("pub struct Row"));
    assert!(a.contains("pub const COUNT: usize = 1;"));
    // keyword field names are escaped (`type` -> `type_`)
    let schema = fs::read_to_string(out1.join("sheets/schema.rs")).unwrap();
    assert!(schema.contains("pub type_:"));
}
