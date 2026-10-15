// SPDX-FileCopyrightText: 2026 Vernum Projecten B.V.
// SPDX-License-Identifier: Apache-2.0

//! The calls `cc-shift` makes to Anthropic: HTTP through the system `curl`, the
//! OAuth refresh and usage calls, and keeping parked accounts ready with
//! them.

pub mod http;
pub mod oauth;
pub mod parked;
