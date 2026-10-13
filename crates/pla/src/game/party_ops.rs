//! Pure party and box operations used by the party screen.
//!
//! The save model remains owned by [`crate::game::save_wire::PlayerProgress`]. This module only
//! validates the party-specific rules before changing its vectors, so the box screen can provide
//! a different UI without sharing any Bevy entities or viewer code.

use std::fmt;

use super::save_game::{PartyMember, MAX_BOX, MAX_PARTY};
use super::save_wire::PlayerProgress;

/// Why a party or box operation was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartyOpError {
    /// A party slot was requested that does not exist.
    BadPartySlot,
    /// A box slot was requested that does not exist.
    BadBoxSlot,
    /// The party has no members.
    EmptyParty,
    /// The party is already at its six-member limit.
    PartyFull,
    /// The box is already at its storage limit.
    BoxFull,
    /// Removing this member would leave no non-fainted party member.
    LastHealthyMember,
    /// The input party already violates the healthy-member invariant.
    NoHealthyMember,
}

impl fmt::Display for PartyOpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::BadPartySlot => "no party member at that slot",
            Self::BadBoxSlot => "no boxed member at that slot",
            Self::EmptyParty => "the party is empty",
            Self::PartyFull => "the party is full",
            Self::BoxFull => "the box is full",
            Self::LastHealthyMember => "at least one non-fainted party member must remain",
            Self::NoHealthyMember => "the party has no non-fainted member",
        };
        f.write_str(message)
    }
}

impl std::error::Error for PartyOpError {}

/// A release destination. Keeping the target explicit avoids coupling this module to a screen's
/// selection or navigation state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReleaseTarget {
    Party(usize),
    Box(usize),
}

/// Return whether at least one party member can still battle.
pub fn has_healthy_member(progress: &PlayerProgress) -> bool {
    progress.party.iter().any(|member| member.hp > 0)
}

/// Validate the party invariant without mutating the save resource.
pub fn validate_party(progress: &PlayerProgress) -> Result<(), PartyOpError> {
    if progress.party.is_empty() {
        return Err(PartyOpError::EmptyParty);
    }
    if !has_healthy_member(progress) {
        return Err(PartyOpError::NoHealthyMember);
    }
    Ok(())
}

/// Reorder two existing party slots.
pub fn reorder_party(
    progress: &mut PlayerProgress,
    from: usize,
    to: usize,
) -> Result<(), PartyOpError> {
    ensure_party_slot(progress, from)?;
    ensure_party_slot(progress, to)?;
    validate_party(progress)?;
    progress.party.swap(from, to);
    Ok(())
}

/// Alias phrased as the operation shown by a party UI.
pub fn reorder(progress: &mut PlayerProgress, from: usize, to: usize) -> Result<(), PartyOpError> {
    reorder_party(progress, from, to)
}

/// Move one party member into the next free box slot.
pub fn move_party_to_box(
    progress: &mut PlayerProgress,
    party_index: usize,
) -> Result<usize, PartyOpError> {
    ensure_party_slot(progress, party_index)?;
    if progress.box_list.len() >= MAX_BOX {
        return Err(PartyOpError::BoxFull);
    }
    if !healthy_after_party_removal(progress, party_index) {
        return Err(PartyOpError::LastHealthyMember);
    }

    let member = progress.party.remove(party_index);
    progress.box_list.push(member);
    Ok(progress.box_list.len() - 1)
}

/// Move one boxed member to the end of the party.
pub fn move_box_to_party(
    progress: &mut PlayerProgress,
    box_index: usize,
) -> Result<(), PartyOpError> {
    ensure_box_slot(progress, box_index)?;
    if progress.party.len() >= MAX_PARTY {
        return Err(PartyOpError::PartyFull);
    }

    // Check the resulting party before removing anything, so every refused operation is atomic.
    let boxed = &progress.box_list[box_index];
    if !has_healthy_member(progress) && boxed.hp == 0 {
        return Err(PartyOpError::NoHealthyMember);
    }

    let member = progress.box_list.remove(box_index);
    progress.party.push(member);
    Ok(())
}

/// Exchange one party member with one boxed member without changing either collection's size.
pub fn swap_party_with_box(
    progress: &mut PlayerProgress,
    party_index: usize,
    box_index: usize,
) -> Result<(), PartyOpError> {
    ensure_party_slot(progress, party_index)?;
    ensure_box_slot(progress, box_index)?;

    let replacing = &progress.box_list[box_index];
    let party_would_be_healthy = progress
        .party
        .iter()
        .enumerate()
        .any(|(index, member)| index != party_index && member.hp > 0)
        || replacing.hp > 0;
    if !party_would_be_healthy {
        return Err(PartyOpError::LastHealthyMember);
    }

    std::mem::swap(
        &mut progress.party[party_index],
        &mut progress.box_list[box_index],
    );
    Ok(())
}

/// Release one party member, refusing to release the last non-fainted member.
pub fn release_party_member(
    progress: &mut PlayerProgress,
    party_index: usize,
) -> Result<PartyMember, PartyOpError> {
    ensure_party_slot(progress, party_index)?;
    if !healthy_after_party_removal(progress, party_index) {
        return Err(PartyOpError::LastHealthyMember);
    }
    Ok(progress.party.remove(party_index))
}

/// Release one boxed member. The party invariant is unaffected.
pub fn release_box_member(
    progress: &mut PlayerProgress,
    box_index: usize,
) -> Result<PartyMember, PartyOpError> {
    ensure_box_slot(progress, box_index)?;
    Ok(progress.box_list.remove(box_index))
}

/// Release a member from either storage location.
pub fn release(
    progress: &mut PlayerProgress,
    target: ReleaseTarget,
) -> Result<PartyMember, PartyOpError> {
    match target {
        ReleaseTarget::Party(index) => release_party_member(progress, index),
        ReleaseTarget::Box(index) => release_box_member(progress, index),
    }
}

fn ensure_party_slot(progress: &PlayerProgress, index: usize) -> Result<(), PartyOpError> {
    if progress.party.get(index).is_some() {
        Ok(())
    } else {
        Err(PartyOpError::BadPartySlot)
    }
}

fn ensure_box_slot(progress: &PlayerProgress, index: usize) -> Result<(), PartyOpError> {
    if progress.box_list.get(index).is_some() {
        Ok(())
    } else {
        Err(PartyOpError::BadBoxSlot)
    }
}

fn healthy_after_party_removal(progress: &PlayerProgress, removed: usize) -> bool {
    progress
        .party
        .iter()
        .enumerate()
        .any(|(index, member)| index != removed && member.hp > 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::save_game::SavedMove;

    fn member(species: u16, hp: u32) -> PartyMember {
        PartyMember {
            species,
            level: 10,
            hp,
            moves: vec![SavedMove {
                move_id: species,
                pp: 10,
            }],
            ..Default::default()
        }
    }

    fn progress(party: &[(u16, u32)], boxed: &[(u16, u32)]) -> PlayerProgress {
        PlayerProgress {
            party: party
                .iter()
                .map(|&(species, hp)| member(species, hp))
                .collect(),
            box_list: boxed
                .iter()
                .map(|&(species, hp)| member(species, hp))
                .collect(),
            ..PlayerProgress::new_game()
        }
    }

    #[test]
    fn reorder_preserves_members_and_order_is_validated() {
        let mut p = progress(&[(1, 10), (2, 0), (3, 20)], &[]);
        reorder_party(&mut p, 0, 2).unwrap();
        assert_eq!(
            p.party.iter().map(|m| m.species).collect::<Vec<_>>(),
            [3, 2, 1]
        );
        assert_eq!(reorder_party(&mut p, 0, 9), Err(PartyOpError::BadPartySlot));
    }

    #[test]
    fn party_to_box_is_atomic_and_box_full_is_reported() {
        let mut p = progress(&[(1, 10), (2, 0)], &[]);
        let slot = move_party_to_box(&mut p, 1).unwrap();
        assert_eq!(slot, 0);
        assert_eq!(p.party.len(), 1);
        assert_eq!(p.box_list[0].species, 2);

        p.box_list = (0..MAX_BOX).map(|i| member(i as u16, 10)).collect();
        let before = p.party.clone();
        assert_eq!(move_party_to_box(&mut p, 0), Err(PartyOpError::BoxFull));
        assert_eq!(p.party, before, "a full box must not mutate the party");
    }

    #[test]
    fn box_to_party_refuses_full_party_and_moves_when_there_is_room() {
        let mut p = progress(&[(1, 10)], &[(2, 20)]);
        move_box_to_party(&mut p, 0).unwrap();
        assert_eq!(
            p.party.iter().map(|m| m.species).collect::<Vec<_>>(),
            [1, 2]
        );
        assert!(p.box_list.is_empty());

        p.party = (0..MAX_PARTY).map(|i| member(i as u16, 10)).collect();
        p.box_list.push(member(99, 10));
        assert_eq!(move_box_to_party(&mut p, 0), Err(PartyOpError::PartyFull));
    }

    #[test]
    fn last_healthy_member_cannot_be_moved_or_released() {
        let mut p = progress(&[(1, 10), (2, 0)], &[]);
        let before = p.clone();
        assert_eq!(
            move_party_to_box(&mut p, 0),
            Err(PartyOpError::LastHealthyMember)
        );
        assert_eq!(p, before);
        assert_eq!(
            release_party_member(&mut p, 0),
            Err(PartyOpError::LastHealthyMember)
        );
        assert_eq!(p, before);
    }

    #[test]
    fn fainted_members_can_be_released_when_health_remains() {
        let mut p = progress(&[(1, 10), (2, 0)], &[(3, 0)]);
        assert_eq!(release_party_member(&mut p, 1).unwrap().species, 2);
        assert_eq!(release_box_member(&mut p, 0).unwrap().species, 3);
        assert!(validate_party(&p).is_ok());
    }

    #[test]
    fn boxed_healthy_member_can_restore_an_invalid_party() {
        let mut p = progress(&[(1, 0)], &[(2, 20)]);
        move_box_to_party(&mut p, 0).unwrap();
        assert!(validate_party(&p).is_ok());
    }

    #[test]
    fn validation_rejects_empty_and_all_fainted_parties() {
        assert_eq!(
            validate_party(&progress(&[], &[])),
            Err(PartyOpError::EmptyParty)
        );
        assert_eq!(
            validate_party(&progress(&[(1, 0), (2, 0)], &[])),
            Err(PartyOpError::NoHealthyMember)
        );
    }
}
