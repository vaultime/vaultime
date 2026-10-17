// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! Play time on the clock: the stretches and quarter hour slices of every
//! session, worked out from the session events, and what the pages show of
//! them, from totals to the journal's days.

pub mod beside;
pub mod journal;
pub mod slices;
pub mod strip;
pub mod summaries;
#[cfg(test)]
pub(crate) mod testing;
pub mod totals;
