//! Starter bag and item menu logic (pure, no Bevy).
//!
//! Item names are not stored in `data/item_effects.tsv`, so items are picked by their decoded
//! effect columns and labelled by what they do (for example "Heal 60 HP #17").

use crate::battle_engine::items::{HpHeal, ItemBag, ItemEffect, ItemTable};
use crate::battle_engine::progression::{ItemProgress, ProgressionTables};
use crate::battle_engine::MajorStatus;

/// Highest item id scanned when looking for items by effect (the sheet has ids up to 1829).
const MAX_SCAN_ID: u16 = 2000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemKind {
    Heal,
    Cure,
    Revive,
    Ball,
    /// Rare candy / effort items (`battle_engine::progression::use_progress_item`).
    Progress,
    Other,
}

pub fn classify(item: &ItemEffect) -> ItemKind {
    if item.is_ball() {
        ItemKind::Ball
    } else if item.effect_kind != 1 {
        ItemKind::Other
    } else if item.revives {
        ItemKind::Revive
    } else if item.hp_heal != HpHeal::None {
        ItemKind::Heal
    } else if !item.cures.is_empty() || item.cures_confusion {
        ItemKind::Cure
    } else {
        ItemKind::Other
    }
}

pub fn status_tag(status: MajorStatus) -> &'static str {
    match status {
        MajorStatus::Drowsy => "SLP",
        MajorStatus::Paralysis => "PAR",
        MajorStatus::Poison => "PSN",
        MajorStatus::Burn => "BRN",
        MajorStatus::Frostbite => "FRB",
    }
}

fn heal_text(h: HpHeal) -> String {
    match h {
        HpHeal::None => "0 HP".into(),
        HpHeal::Fixed(n) => format!("{n} HP"),
        HpHeal::Fraction(n, d) => format!("{n}/{d} HP"),
        HpHeal::Full => "full HP".into(),
    }
}

/// Short menu label built from the decoded effect.
pub fn label(item: &ItemEffect) -> String {
    let id = item.item_id;
    match classify(item) {
        ItemKind::Heal => {
            let mut s = format!("Heal {}", heal_text(item.hp_heal));
            if item.cures.len() >= 5 {
                s.push_str(" + cure all");
            }
            format!("{s} #{id}")
        }
        ItemKind::Cure if item.cures.len() >= 5 => format!("Cure all #{id}"),
        ItemKind::Cure => {
            let tags: Vec<&str> = item.cures.iter().map(|s| status_tag(*s)).collect();
            format!("Cure {} #{id}", tags.join("/"))
        }
        ItemKind::Revive => format!("Revive {} #{id}", heal_text(item.hp_heal)),
        ItemKind::Ball => format!(
            "{} Ball #{id}",
            item.ball_category.as_deref().unwrap_or("?")
        ),
        ItemKind::Progress | ItemKind::Other => format!("Item #{id}"),
    }
}

/// One selectable bag line: item id plus the label shown in the menu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BagEntry {
    pub item_id: u16,
    pub label: String,
    pub kind: ItemKind,
}

/// Items chosen by effect, in menu order, with starter quantities.
/// The rule for each pick is "lowest item id that matches", so it is deterministic.
pub fn starter_selection(table: &ItemTable) -> Vec<(u16, u16)> {
    let items: Vec<&ItemEffect> = (1..=MAX_SCAN_ID).filter_map(|id| table.get(id)).collect();
    let first = |pred: &dyn Fn(&ItemEffect) -> bool| -> Option<u16> {
        items.iter().find(|i| pred(i)).map(|i| i.item_id)
    };
    let pure_fixed = |i: &ItemEffect| {
        classify(i) == ItemKind::Heal && i.cures.is_empty() && matches!(i.hp_heal, HpHeal::Fixed(_))
    };
    let fixed = |i: &ItemEffect| match i.hp_heal {
        HpHeal::Fixed(n) => n,
        _ => 0,
    };
    let mut out: Vec<(u16, u16)> = Vec::new();
    let mut push = |id: Option<u16>, qty: u16| {
        if let Some(id) = id {
            if !out.iter().any(|(i, _)| *i == id) {
                out.push((id, qty));
            }
        }
    };
    // Weakest and strongest plain HP restorers (e.g. 60 HP and 150 HP potions).
    push(first(&pure_fixed), 5);
    let strongest = items
        .iter()
        .filter(|i| pure_fixed(i))
        .max_by_key(|i| (fixed(i), std::cmp::Reverse(i.item_id)))
        .map(|i| i.item_id);
    push(strongest, 2);
    for status in [MajorStatus::Poison, MajorStatus::Burn, MajorStatus::Paralysis] {
        push(
            first(&|i| {
                classify(i) == ItemKind::Cure
                    && i.hp_heal == HpHeal::None
                    && i.cures == [status]
                    && !i.cures_confusion
            }),
            2,
        );
    }
    push(
        first(&|i| classify(i) == ItemKind::Cure && i.cures.len() >= 5 && i.cures_confusion),
        1,
    );
    push(
        first(&|i| classify(i) == ItemKind::Revive && i.hp_heal == HpHeal::Fraction(1, 2)),
        1,
    );
    push(
        first(&|i| i.is_ball() && i.ball_category.as_deref() == Some("Regular")),
        10,
    );
    push(
        first(&|i| i.is_ball() && i.ball_category.as_deref() == Some("Great")),
        5,
    );
    out
}

const EFFORT_TAGS: [&str; 6] = ["HP", "Atk", "Def", "SpA", "SpD", "Spe"];

/// Is this a progression item the bag can use (level-up or effort change)? Evolution items
/// are not handled in battle.
pub fn is_progress_item(p: &ItemProgress) -> bool {
    p.level_up || p.effort_amount.iter().any(|a| *a != 0)
}

/// Menu label of a progression item: "Level +1 #50", "Effort Atk+ #46", "Effort HP- #169".
pub fn progress_label(item_id: u16, p: &ItemProgress) -> String {
    if p.level_up {
        return format!("Level +1 #{item_id}");
    }
    let parts: Vec<String> = p
        .effort_amount
        .iter()
        .zip(EFFORT_TAGS)
        .filter(|(a, _)| **a != 0)
        .map(|(a, tag)| format!("{tag}{}", if *a > 0 { '+' } else { '-' }))
        .collect();
    format!("Effort {} #{item_id}", parts.join(" "))
}

/// Bag entry for any saved item id: progression items first (they need `prog`), otherwise the
/// effect label. `None` when the item table does not know the id.
pub fn entry_for(item_id: u16, table: &ItemTable, prog: Option<&ProgressionTables>) -> Option<BagEntry> {
    let item = table.get(item_id)?;
    if let Some(p) = prog.and_then(|t| t.item(item_id)).filter(is_progress_item) {
        return Some(BagEntry { item_id, label: progress_label(item_id, &p), kind: ItemKind::Progress });
    }
    Some(BagEntry { item_id, label: label(item), kind: classify(item) })
}

/// Starter progression items (deterministic, lowest id first): the first level-up item x3
/// (rare candy, #50), the first two effort-raising items x2 and the first effort-lowering
/// item x1. Only ids the item table knows are picked (the bag needs their stack limit).
pub fn progress_selection(table: &ItemTable, prog: &ProgressionTables) -> Vec<(u16, u16)> {
    let items: Vec<(u16, ItemProgress)> = (1..=MAX_SCAN_ID)
        .filter(|id| table.get(*id).is_some())
        .filter_map(|id| prog.item(id).filter(is_progress_item).map(|p| (id, p)))
        .collect();
    let mut out = Vec::new();
    if let Some((id, _)) = items.iter().find(|(_, p)| p.level_up) {
        out.push((*id, 3));
    }
    let raise = |p: &ItemProgress| !p.level_up && p.effort_amount.iter().any(|a| *a > 0);
    out.extend(items.iter().filter(|(_, p)| raise(p)).take(2).map(|(id, _)| (*id, 2)));
    let lower = |p: &ItemProgress| !p.level_up && p.effort_amount.iter().any(|a| *a < 0);
    out.extend(items.iter().filter(|(_, p)| lower(p)).take(1).map(|(id, _)| (*id, 1)));
    out
}

/// Add the progression starter items to a bag and its menu order.
pub fn add_progress_items(bag: &mut ItemBag, order: &mut Vec<BagEntry>, table: &ItemTable, prog: &ProgressionTables) {
    for (id, qty) in progress_selection(table, prog) {
        if order.iter().any(|e| e.item_id == id) {
            continue;
        }
        if bag.add(table, id, qty).is_ok() {
            if let Some(p) = prog.item(id) {
                order.push(BagEntry { item_id: id, label: progress_label(id, &p), kind: ItemKind::Progress });
            }
        }
    }
}

/// Build the engine bag and the menu order from the starter selection.
pub fn starter_bag(table: &ItemTable) -> (ItemBag, Vec<BagEntry>) {
    let mut bag = ItemBag::default();
    let mut order = Vec::new();
    for (id, qty) in starter_selection(table) {
        if bag.add(table, id, qty).is_ok() {
            let item = table.get(id).expect("selected from the table");
            order.push(BagEntry {
                item_id: id,
                label: label(item),
                kind: classify(item),
            });
        }
    }
    (bag, order)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fx(id: u16) -> ItemEffect {
        ItemEffect {
            item_id: id,
            price: 100,
            max_quantity: 999,
            effect_kind: 1,
            pocket_code: 6,
            storage_class: 0,
            ball_category: None,
            hp_heal: HpHeal::None,
            cures: vec![],
            cures_confusion: false,
            revives: false,
            pp_restore_one: false,
            pp_restore_all: false,
            pp_amount: 0,
            level_up: false,
        }
    }

    fn table() -> ItemTable {
        let mut potion = fx(17);
        potion.hp_heal = HpHeal::Fixed(60);
        let mut hyper = fx(25);
        hyper.hp_heal = HpHeal::Fixed(150);
        let mut antidote = fx(18);
        antidote.cures = vec![MajorStatus::Poison];
        let mut burn = fx(19);
        burn.cures = vec![MajorStatus::Burn];
        let mut para = fx(22);
        para.cures = vec![MajorStatus::Paralysis];
        let mut full = fx(27);
        full.cures = vec![
            MajorStatus::Drowsy,
            MajorStatus::Poison,
            MajorStatus::Burn,
            MajorStatus::Frostbite,
            MajorStatus::Paralysis,
        ];
        full.cures_confusion = true;
        let mut revive = fx(28);
        revive.hp_heal = HpHeal::Fraction(1, 2);
        revive.revives = true;
        let mut ball = fx(4);
        ball.effect_kind = 0;
        ball.ball_category = Some("Regular".into());
        let mut great = fx(3);
        great.effect_kind = 0;
        great.ball_category = Some("Great".into());
        ItemTable::from_items([potion, hyper, antidote, burn, para, full, revive, ball, great])
    }

    #[test]
    fn picks_items_by_effect_in_menu_order() {
        let sel = starter_selection(&table());
        let ids: Vec<u16> = sel.iter().map(|(i, _)| *i).collect();
        assert_eq!(ids, vec![17, 25, 18, 19, 22, 27, 28, 4, 3]);
        assert_eq!(sel[0].1, 5);
        assert_eq!(sel[7].1, 10);
    }

    #[test]
    fn bag_quantities_come_from_the_engine_bag() {
        let t = table();
        let (mut bag, order) = starter_bag(&t);
        assert_eq!(order.len(), 9);
        assert_eq!(bag.count(17), 5);
        assert!(bag.take(17));
        assert_eq!(bag.count(17), 4);
        assert_eq!(order[0].kind, ItemKind::Heal);
        assert_eq!(order[7].kind, ItemKind::Ball);
    }

    #[test]
    fn labels_describe_the_effect() {
        let t = table();
        assert_eq!(label(t.get(17).unwrap()), "Heal 60 HP #17");
        assert_eq!(label(t.get(18).unwrap()), "Cure PSN #18");
        assert_eq!(label(t.get(27).unwrap()), "Cure all #27");
        assert_eq!(label(t.get(28).unwrap()), "Revive 1/2 HP #28");
        assert_eq!(label(t.get(4).unwrap()), "Regular Ball #4");
    }

    #[test]
    fn progress_labels_describe_level_and_effort() {
        let candy = ItemProgress { level_up: true, ..ItemProgress::default() };
        assert_eq!(progress_label(50, &candy), "Level +1 #50");
        let up = ItemProgress { effort_amount: [0, 10, 0, 0, 0, 0], ..ItemProgress::default() };
        assert_eq!(progress_label(46, &up), "Effort Atk+ #46");
        let down = ItemProgress { effort_amount: [-10, 0, 0, 0, 0, 0], ..ItemProgress::default() };
        assert_eq!(progress_label(169, &down), "Effort HP- #169");
        assert!(is_progress_item(&candy) && is_progress_item(&down));
        assert!(!is_progress_item(&ItemProgress { evolution_item: true, ..ItemProgress::default() }));
    }

    /// Real tables: rare candy (#50, VERIFIED level_up column) is picked, plus effort items.
    #[test]
    fn real_progress_selection_has_rare_candy_and_effort_items() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let (Ok(table), Ok(prog)) = (ItemTable::load(&root), ProgressionTables::load(&root)) else { return };
        let sel = progress_selection(&table, &prog);
        assert_eq!(sel.first(), Some(&(50, 3)), "{sel:?}");
        assert!(sel.len() >= 3, "{sel:?}");
        let mut bag = ItemBag::default();
        let mut order = Vec::new();
        add_progress_items(&mut bag, &mut order, &table, &prog);
        assert_eq!(bag.count(50), 3);
        assert!(order.iter().all(|e| e.kind == ItemKind::Progress));
        let e = entry_for(50, &table, Some(&prog)).unwrap();
        assert_eq!((e.kind, e.label.as_str()), (ItemKind::Progress, "Level +1 #50"));
    }

    #[test]
    fn empty_table_gives_empty_bag() {
        let (bag, order) = starter_bag(&ItemTable::default());
        assert!(order.is_empty());
        assert_eq!(bag.count(17), 0);
    }
}
