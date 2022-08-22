/*
 * SPDX-FileCopyrightText: 2022 perillamint
 *
 * SPDX-License-Identifier: MPL-2.0
 *
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

/// Flipper Zero max frame length.
/// Value from flipper firmware applications/rpc/rpc.h
pub const MAX_FRAME_LENGTH: usize = 1536;
/// Flipper zero prompt pattern in u8 slice.
/// Human readable representation: '\n>: '
pub const PROMPT_PATTERN: [u8; 4] = [0x0a, 0x3e, 0x3a, 0x20];
