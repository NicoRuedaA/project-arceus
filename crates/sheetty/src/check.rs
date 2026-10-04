//! Preflight layers L0-L3 (The Spreadsheet Method, section 6).

use std::collections::{BTreeMap, BTreeSet};

use crate::parse::Sheet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Debug, Clone)]
pub struct Finding {
    pub code: String,
    pub location: String,
    pub message: String,
    pub severity: Severity,
}

impl Finding {
    fn error(code: &str, location: impl Into<String>, message: impl Into<String>) -> Finding {
        Finding {
            code: code.to_string(),
            location: location.into(),
            message: message.into(),
            severity: Severity::Error,
        }
    }
    fn warning(code: &str, location: impl Into<String>, message: impl Into<String>) -> Finding {
        Finding {
            code: code.to_string(),
            location: location.into(),
            message: message.into(),
            severity: Severity::Warning,
        }
    }
}

fn valid_id(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

fn valid_type(ty: &str) -> bool {
    matches!(
        ty,
        "string" | "u8" | "u16" | "u32" | "u64" | "i32" | "i64" | "f32" | "f64" | "bool" | "enum"
    )
}

fn type_ok(ty: &str, value: &str) -> bool {
    match ty {
        "u8" => value.parse::<u8>().is_ok(),
        "u16" => value.parse::<u16>().is_ok(),
        "u32" => value.parse::<u32>().is_ok(),
        "u64" => value.parse::<u64>().is_ok(),
        "i32" => value.parse::<i32>().is_ok(),
        "i64" => value.parse::<i64>().is_ok(),
        "f32" => value.parse::<f32>().is_ok(),
        "f64" => value.parse::<f64>().is_ok(),
        "bool" => value == "0" || value == "1",
        _ => true,
    }
}

/// Run the L0-L3 preflight over a loaded sheet book.
pub fn check(sheets: &[Sheet]) -> Vec<Finding> {
    let mut findings = Vec::new();

    // Index of sheet name -> id set (for L2) and plan/impl (for L3).
    let mut ids: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut plan: BTreeSet<String> = BTreeSet::new();
    let mut imp: BTreeSet<String> = BTreeSet::new();

    for sheet in sheets {
        let name = sheet.name().to_string();
        let loc = sheet.path.display().to_string();

        // L0: manifest.
        for key in ["sheet", "version", "generator", "target"] {
            if !sheet.manifest.contains_key(key) {
                findings.push(Finding::error(
                    "E-L0-MANIFEST",
                    loc.clone(),
                    format!("missing manifest key `{key}`"),
                ));
            }
        }
        // L0: delimiter / column grammar.
        if sheet.columns.len() < 2 {
            findings.push(Finding::error(
                "E-L0-DELIM",
                loc.clone(),
                "header row has fewer than two tab-separated columns",
            ));
        }
        for col in &sheet.columns {
            if !valid_type(&col.ty) {
                findings.push(Finding::error(
                    "E-L1-TYPE",
                    format!("{name}:1"),
                    format!("column `{}` has unknown type `{}`", col.name, col.ty),
                ));
            }
            if !col
                .name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_')
            {
                findings.push(Finding::error(
                    "E-L0-COLNAME",
                    format!("{name}:1"),
                    format!("bad column name `{}`", col.name),
                ));
            }
        }

        let mut seen: BTreeSet<String> = BTreeSet::new();
        for row in &sheet.rows {
            let line = row.line;
            let id = sheet.cell(row, "id").to_string();
            if sheet.column_index("id").is_some() {
                if id.is_empty() {
                    findings.push(Finding::error(
                        "E-L0-KEY",
                        format!("{name}:{line}"),
                        "empty id",
                    ));
                } else if !valid_id(&id) {
                    findings.push(Finding::error(
                        "E-L0-KEYCHARS",
                        format!("{name}:{line}"),
                        format!("id `{id}` does not match ^[a-z0-9_]+$"),
                    ));
                } else if !seen.insert(id.clone()) {
                    findings.push(Finding::error(
                        "E-L0-DUPKEY",
                        format!("{name}:{line}"),
                        format!("duplicate id `{id}`"),
                    ));
                }
            }
            for (i, col) in sheet.columns.iter().enumerate() {
                let value = row.get(i);
                if i >= row.cells.len() {
                    findings.push(Finding::error(
                        "E-L1-SHORT",
                        format!("{name}:{line}"),
                        format!("missing cell for `{}`", col.name),
                    ));
                    continue;
                }
                if (col.flags.contains('!') || col.name == "id") && value.is_empty() {
                    findings.push(Finding::error(
                        "E-L1-REQUIRED",
                        format!("{name}:{line}"),
                        format!("`{}` is required but empty", col.name),
                    ));
                }
                if !value.is_empty() && value != "NULL" && !type_ok(&col.ty, value) {
                    findings.push(Finding::error(
                        "E-L1-TYPE",
                        format!("{name}:{line}"),
                        format!("`{}` = `{value}` is not {}", col.name, col.ty),
                    ));
                }
            }
        }
        if name == "02-plan" {
            plan = seen.clone();
        }
        if name == "03-impl" {
            imp = seen.clone();
        }
        ids.insert(name.clone(), seen);
    }

    // L2: reference integrity.
    for sheet in sheets {
        let name = sheet.name();
        for (i, col) in sheet.columns.iter().enumerate() {
            let Some((target_sheet, _target_col)) = &col.reference else {
                continue;
            };
            let pool = ids.get(target_sheet.as_str());
            for row in &sheet.rows {
                let value = row.get(i);
                if value.is_empty() || value == "NULL" {
                    continue;
                }
                let found = pool.map(|p| p.contains(value)).unwrap_or(false);
                if !found {
                    findings.push(Finding::error(
                        "E-L2-REF",
                        format!("{}:{}", name, row.line),
                        format!("`{}` = `{value}` not found in {target_sheet}", col.name),
                    ));
                }
            }
        }
    }

    // L3: coverage (plan vs impl).
    for id in plan.difference(&imp) {
        findings.push(Finding::warning(
            "W-L3-UNIMPLEMENTED",
            "02-plan",
            id.clone(),
        ));
    }
    for id in imp.difference(&plan) {
        findings.push(Finding::warning("W-L3-ORPHAN", "03-impl", id.clone()));
    }

    findings
}
