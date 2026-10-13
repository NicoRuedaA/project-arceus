//! Windowless report on the decoded door pairs (Brief 13, door part).
//! `cargo run -p pla --example door_pairs_report`
use pla::game::door_pairs::{DestKind, DoorPairIndex};
use pla::game::zone::ZoneLinks;

fn main() {
    let links = ZoneLinks::load_default();
    if links.is_empty() {
        eprintln!("[skip] data/warp_links.tsv not found");
        return;
    }
    let idx = DoorPairIndex::build(&links);
    println!("{:#?}", idx.stats());
    for p in idx.pairs() {
        if !p.reciprocal || p.arrival_in_exit_box || !p.exit_found {
            println!(
                "note: {} -> {:?} reciprocal={} arrival_in_exit_box={} dest={}",
                p.door,
                p.exit,
                p.reciprocal,
                p.arrival_in_exit_box,
                match p.dest_kind {
                    DestKind::MainArea => "main",
                    DestKind::SubArea => "sub",
                }
            );
        }
    }
}
