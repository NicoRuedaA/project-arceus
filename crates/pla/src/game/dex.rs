//! Pokédex research ranks (brief 43, its data phase).
//!
//! What IS decoded (PROBABLE, see [`RANKS`] and `data/dex_ranks.tsv`): the player's research rank
//! ladder — the research points each rank needs — recovered from the game's own
//! `bin/appli/pokedex/res_table/pokedex_rank_table.bin` by `.tools/dex_research_probe.py`. The
//! nine rank records repeat a monotone `(rank, points)` shape, which is what makes the reading
//! structural rather than a guess; rank 1 has no readable threshold (its row stays UNKNOWN).
//!
//! What is NOT decoded (UNKNOWN, do not invent it): the per-species research tasks, their target
//! counts, their point values and the points a capture / defeat / observation awards. The
//! game's research-task table announces 1611 task records and its records are 48..112-byte
//! blocks with three 64-bit hashes, but neither the per-species grouping nor the field meaning
//! has been established, so no `data/dex_tasks.tsv` exists and this module awards no points by
//! itself: a caller that has points (from a future decoder or a test) asks for the rank.

/// `(rank id, research points needed)`, ascending. PROBABLE: nine records of the rank table form
/// a monotone ladder (rank 10 needs 60000 points, rank 2 needs 1800).
pub const RANKS: [(u8, u32); 9] = [
    (2, 1800),
    (3, 3500),
    (4, 6000),
    (5, 8500),
    (6, 11000),
    (7, 15000),
    (8, 20000),
    (9, 30000),
    (10, 60000),
];

/// Research rank for a number of research points: the highest decoded rank whose threshold is
/// reached. Below the lowest decoded threshold the rank is 1 (SUPUESTO: the game's rank 1
/// threshold has no readable record).
pub fn rank_for_points(points: u32) -> u8 {
    RANKS
        .iter()
        .rev()
        .find(|(_, needed)| points >= *needed)
        .map_or(1, |(rank, _)| *rank)
}

/// Research points the next rank needs, `None` at the top rank.
pub fn next_rank_at(points: u32) -> Option<(u8, u32)> {
    RANKS
        .iter()
        .find(|(_, needed)| points < *needed)
        .copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ladder_is_sorted_unique_and_monotone() {
        assert_eq!(RANKS.len(), 9);
        for pair in RANKS.windows(2) {
            assert!(pair[0].0 < pair[1].0, "ranks ascending");
            assert!(pair[0].1 < pair[1].1, "points ascending");
        }
    }

    #[test]
    fn rank_boundaries_round_down_to_the_reached_rank() {
        assert_eq!(rank_for_points(0), 1, "SUPUESTO floor below the first decoded rank");
        assert_eq!(rank_for_points(1799), 1);
        assert_eq!(rank_for_points(1800), 2);
        assert_eq!(rank_for_points(59999), 9);
        assert_eq!(rank_for_points(60000), 10);
        assert_eq!(rank_for_points(u32::MAX), 10, "the top rank holds");
        assert_eq!(next_rank_at(0), Some((2, 1800)));
        assert_eq!(next_rank_at(1800), Some((3, 3500)));
        assert_eq!(next_rank_at(60000), None);
    }

    /// The code table and the tracked sheet are the same data (the sheet carries the provenance:
    /// file and offset of each probed record).
    #[test]
    fn the_model_matches_the_tracked_sheet() {
        let sheet = include_str!("../../../../data/dex_ranks.tsv");
        let mut rows = Vec::new();
        for line in sheet.lines().filter(|l| !l.starts_with('#') && !l.trim().is_empty()) {
            let fields: Vec<&str> = line.split('\t').collect();
            if fields.first() == Some(&"rank:u8") {
                continue; // typed header
            }
            assert_eq!(fields.len(), 5, "row shape: {line}");
            assert_eq!(fields[2], "PROBABLE", "every row keeps its confidence: {line}");
            assert_eq!(fields[3], "pokedex_rank_table.bin");
            rows.push((fields[0].parse::<u8>().unwrap(), fields[1].parse::<u32>().unwrap()));
        }
        assert_eq!(rows, RANKS);
    }
}
