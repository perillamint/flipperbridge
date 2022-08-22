/*
 * SPDX-FileCopyrightText: 2022 perillamint
 *
 * SPDX-License-Identifier: MPL-2.0
 *
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

use super::error::FlipperError;
use crate::consts::MAX_FRAME_LENGTH;
use async_trait::async_trait;
use integer_encoding::VarInt;

#[cfg(feature = "ble")]
pub mod ble;
#[cfg(feature = "serial")]
pub mod serial;

/// Transport interface definition
#[async_trait]
pub trait FlipperTransport {
    /// Initialize and prepare serial stream for FZ RPC communication.
    /// Must be called before start sending / receiving RPC command frames.
    async fn init(&mut self) -> Result<(), FlipperError>;
    /// Read FZ RPC frame. Returns frame body without frame header(length)
    async fn read_frame(&mut self) -> Result<Vec<u8>, FlipperError>;
    /// Write(send) FZ RPC frame. Frame header will be automatically calculated and appended.
    async fn write_frame(&mut self, data: &[u8]) -> Result<(), FlipperError>;
}
