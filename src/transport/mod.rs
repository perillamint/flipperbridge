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
use async_trait::async_trait;

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

    /// Split stream into two separated stream.
    async fn split_stream(self) -> (Box<dyn FlipperFrameReceiver>, Box<dyn FlipperFrameSender>);
}

#[async_trait]
pub trait FlipperFrameSender {
    /// Write(send) FZ RPC frame. Frame header will be automatically calculated and appended.
    async fn write_frame(&mut self, data: &[u8]) -> Result<(), FlipperError>;
}

#[async_trait]
pub trait FlipperFrameReceiver {
    /// Read FZ RPC frame. Returns frame body without frame header(length)
    async fn read_frame(&mut self) -> Result<Vec<u8>, FlipperError>;
}
