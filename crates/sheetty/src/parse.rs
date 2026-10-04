//! Canonical TSV parsing (The Spreadsheet Method, section 5.2).

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

/// A parsed column header: `name:type[?] [unit] [-> sheet.col] [= default] [*flags]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Column {
    pub name: String,
    pub ty: String,
    pub optional: bool,
    pub reference: Option<(String, String)>,
    pub default: Option<String>,
    pub flags: String,
}

impl Column {
    pub fn parse(raw: &str) -> Column {
        let (name, rest) = match raw.split_once(':') {
            Some((n, r)) => (n.trim().to_string(), r.trim().to_string()),
            None => (raw.trim().to_string(), String::new()),
        };
        let mut ty = String::new();
        let mut optional = false;
        let mut reference = None;
        let mut default = None;
        let mut flags = String::new();
        let mut tokens = rest.split_whitespace().peekable();
        if let Some(first) = tokens.next() {
            let mut t = first.to_string();
            if t.ends_with('?') {
                optional = true;
                t.pop();
            }
            if let Some(pos) = t.find('*') {
                flags = t[pos..].to_string();
                t.truncate(pos);
            }
            ty = t;
        }
        while let Some(tok) = tokens.next() {
            if let Some(target) = tok.strip_prefix("->") {
                let target = if target.is_empty() {
                    tokens.next().unwrap_or("").to_string()
                } else {
                    target.to_string()
                };
                if let Some((s, c)) = target.split_once('.') {
                    reference = Some((s.to_string(), c.to_string()));
                }
            } else if let Some(d) = tok.strip_prefix('=') {
                default = Some(if d.is_empty() {
                    tokens.next().unwrap_or("").to_string()
                } else {
                    d.to_string()
                });
            } else if tok.starts_with('*') {
                flags = tok.to_string();
            }
        }
        Column {
            name,
            ty,
            optional,
            reference,
            default,
            flags,
        }
    }

    pub fn is_primary_key(&self) -> bool {
        self.name == "id" && self.flags.contains('*')
    }
}

/// One data row; cells are kept in column order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub cells: Vec<String>,
    pub line: usize,
}

impl Row {
    pub fn get(&self, index: usize) -> &str {
        self.cells.get(index).map(|s| s.as_str()).unwrap_or("")
    }
}

/// A parsed sheet.
#[derive(Debug, Clone)]
pub struct Sheet {
    pub path: PathBuf,
    pub manifest: BTreeMap<String, String>,
    pub columns: Vec<Column>,
    pub rows: Vec<Row>,
}

impl Sheet {
    pub fn name(&self) -> &str {
        self.manifest.get("sheet").map(|s| s.as_str()).unwrap_or("")
    }

    pub fn column_index(&self, name: &str) -> Option<usize> {
        self.columns.iter().position(|c| c.name == name)
    }

    pub fn cell<'a>(&self, row: &'a Row, name: &str) -> &'a str {
        match self.column_index(name) {
            Some(i) => row.get(i),
            None => "",
        }
    }

    /// Whether this sheet participates in emission (`# emit: false` opts out).
    pub fn emits(&self) -> bool {
        self.manifest
            .get("emit")
            .map(|v| v != "false")
            .unwrap_or(true)
    }
}

/// Load every `.tsv` under `dir` (recursive), sorted by path for determinism.
pub fn load_dir(dir: &Path) -> Result<Vec<Sheet>, String> {
    let mut paths = Vec::new();
    collect(dir, &mut paths).map_err(|e| format!("{}: {e}", dir.display()))?;
    paths.sort();
    let mut sheets = Vec::new();
    for p in paths {
        sheets.push(parse_file(&p)?);
    }
    Ok(sheets)
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect(&path, out)?;
        } else if path.extension().map(|e| e == "tsv").unwrap_or(false) {
            out.push(path);
        }
    }
    Ok(())
}

/// Parse one sheet. Structural violations are reported as errors here; the
/// check layer adds type, reference, and coverage checks.
pub fn parse_file(path: &Path) -> Result<Sheet, String> {
    let bytes = fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        return Err(format!("{}: E-L0-BOM (UTF-8 BOM present)", path.display()));
    }
    if bytes.contains(&b'\r') {
        return Err(format!(
            "{}: E-L0-EOL (CR line ending present)",
            path.display()
        ));
    }
    let text =
        String::from_utf8(bytes).map_err(|e| format!("{}: not UTF-8: {e}", path.display()))?;

    let mut manifest = BTreeMap::new();
    let mut columns: Option<Vec<Column>> = None;
    let mut rows = Vec::new();
    for (idx, line) in text.lines().enumerate() {
        let line_no = idx + 1;
        if columns.is_none() {
            if let Some(rest) = line.strip_prefix('#') {
                if let Some((k, v)) = rest.split_once(':') {
                    let value = v.split('#').next().unwrap_or("").trim().to_string();
                    manifest.insert(k.trim().to_string(), value);
                }
                continue;
            }
            if line.is_empty() {
                continue;
            }
            let cols: Vec<Column> = line.split('\t').map(Column::parse).collect();
            columns = Some(cols);
            continue;
        }
        if line.is_empty() || line.starts_with("//") {
            continue;
        }
        rows.push(Row {
            cells: line.split('\t').map(|s| s.to_string()).collect(),
            line: line_no,
        });
    }

    let columns =
        columns.ok_or_else(|| format!("{}: E-L0-EMPTY (no header row)", path.display()))?;
    Ok(Sheet {
        path: path.to_path_buf(),
        manifest,
        columns,
        rows,
    })
}
