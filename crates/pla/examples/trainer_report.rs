//! Windowless summary of the decoded trainer and noble sheets.
//!
//! Reads `data/trainer_index.tsv`, `data/trainer_teams.tsv` and `data/boss_battles.tsv`
//! (committed sheets, no game files needed) and prints counts per trainer group, AI module
//! coverage, party sizes and the noble list. Usage:
//! `cargo run -p pla --example trainer_report [-- <trainer_id>]`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

struct Sheet {
    cols: Vec<String>,
    rows: Vec<Vec<String>>,
}

impl Sheet {
    fn load(path: &Path) -> Result<Sheet, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut lines = text
            .lines()
            .filter(|l| !l.starts_with('#') && !l.is_empty());
        let cols = lines
            .next()
            .ok_or_else(|| format!("{}: no header", path.display()))?
            .split('\t')
            .map(|c| c.split(':').next().unwrap_or("").to_string())
            .collect();
        let rows = lines
            .map(|l| l.split('\t').map(str::to_string).collect())
            .collect();
        Ok(Sheet { cols, rows })
    }

    fn col(&self, name: &str) -> usize {
        self.cols
            .iter()
            .position(|c| c == name)
            .unwrap_or_else(|| panic!("missing column {name}"))
    }
}

fn data_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")
}

fn main() -> Result<(), String> {
    let dir = data_dir();
    let index = Sheet::load(&dir.join("trainer_index.tsv"))?;
    let teams = Sheet::load(&dir.join("trainer_teams.tsv"))?;
    let bosses = Sheet::load(&dir.join("boss_battles.tsv"))?;

    let (g, label, model, size) = (
        index.col("group"),
        index.col("trname_label"),
        index.col("model_pack"),
        index.col("party_size"),
    );
    println!(
        "trainers: {}  party members: {}",
        index.rows.len(),
        teams.rows.len()
    );

    let mut groups: BTreeMap<&str, (usize, &str, &str)> = BTreeMap::new();
    for r in &index.rows {
        let e = groups
            .entry(r[g].as_str())
            .or_insert((0, r[label].as_str(), r[model].as_str()));
        e.0 += 1;
    }
    println!(
        "\n{:<12} {:>5}  {:<18} model",
        "group", "files", "trname label"
    );
    for (k, (n, l, m)) in &groups {
        println!(
            "{k:<12} {n:>5}  {:<18} {m}",
            if l.is_empty() { "?" } else { l }
        );
    }

    let ai = [
        "ai_basic",
        "ai_strong",
        "ai_expert",
        "ai_allowance",
        "ai_item",
        "ai_change",
    ];
    let ai_idx: Vec<usize> = ai.iter().map(|c| index.col(c)).collect();
    let mut profiles: BTreeMap<String, usize> = BTreeMap::new();
    for r in &index.rows {
        let p: Vec<&str> = ai
            .iter()
            .zip(&ai_idx)
            .filter(|(_, &i)| r[i] == "1")
            .map(|(n, _)| &n[3..])
            .collect();
        *profiles.entry(p.join("+")).or_default() += 1;
    }
    println!("\nAI module profiles (LIKELY mapping of trainer slots 16..21):");
    for (p, n) in &profiles {
        println!("  {n:>4}  {p}");
    }

    let mut sizes: BTreeMap<&str, usize> = BTreeMap::new();
    for r in &index.rows {
        *sizes.entry(r[size].as_str()).or_default() += 1;
    }
    println!("\nparty sizes: {sizes:?}");

    let (bid, sp, lv, gauge, balm) = (
        bosses.col("id"),
        bosses.col("species"),
        bosses.col("level"),
        bosses.col("gauge_max"),
        bosses.col("balm_item"),
    );
    println!("\nnoble settings: {}", bosses.rows.len());
    for r in &bosses.rows {
        println!(
            "  {:<13} {:<10} L{:<4} gauge {:<6} balm {}",
            r[bid], r[sp], r[lv], r[gauge], r[balm]
        );
    }

    if let Some(want) = std::env::args().nth(1) {
        let (tid, slot, species, level) = (
            teams.col("trainer_id"),
            teams.col("slot"),
            teams.col("species"),
            teams.col("level"),
        );
        let moves: Vec<usize> = (1..=4).map(|k| teams.col(&format!("move{k}"))).collect();
        println!("\nparty of {want}:");
        for r in teams.rows.iter().filter(|r| r[tid] == want) {
            let mv: Vec<&str> = moves
                .iter()
                .map(|&i| r[i].as_str())
                .filter(|m| !m.is_empty())
                .collect();
            println!(
                "  #{} {} L{} [{}]",
                r[slot],
                r[species],
                r[level],
                mv.join(", ")
            );
        }
    }
    Ok(())
}
