// SPDX-FileCopyrightText: 2026 Vernum Projecten B.V.
// SPDX-License-Identifier: Apache-2.0

//! Where `cc-shift` plugs into Claude Code: its settings file, its status line,
//! and starting `claude` with a profile's config folder.

pub mod launch;
pub mod refresh_lock;
pub mod settings;
pub mod statusline;
