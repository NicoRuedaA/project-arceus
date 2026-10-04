//! Behavior dispatch without `dyn` (The Spreadsheet Method, section 5.7,
//! mechanism 3): a contiguous table of non-capturing function pointers indexed
//! by the generated dense index. Stubs until a subsystem is actually ported;
//! the table shape is what matters.

use crate::app::PortContext;
use crate::generated::domain_subsystems::{Index, COUNT};

pub type Behavior = fn(&mut PortContext, Index);

/// Placeholder behavior; replace per subsystem as the port progresses.
pub fn subsystem_stub(_ctx: &mut PortContext, _index: Index) {}

pub static BEHAVIORS: [Behavior; COUNT] = [subsystem_stub; COUNT];
