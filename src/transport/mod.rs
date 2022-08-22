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

/// Stream packetizer utility
struct StreamPacketizer {
    buf: Vec<u8>,
}

impl StreamPacketizer {
    /// Create new stream packetizer instance
    pub fn new() -> Self {
        Self { buf: vec![] }
    }

    /// Pour data from raw stream. Provided data will be stored
    /// in the internal buffer and will be used later when poll() called.
    pub fn fill_buffer(&mut self, data: &[u8]) {
        self.buf.extend_from_slice(data)
    }

    /// Grab FZ RPC frame from the internal buffer.
    /// Will return Ok(Some(Vec<u8>)) when it has data ready to provide
    /// and will return Ok(None) when it didn't received whole frame yet.
    /// Will return DataTooLarge error when packetizer encounters
    /// frame larger then MAX_FRAME_LENGTH.
    pub fn poll(&mut self) -> Result<Option<Vec<u8>>, FlipperError> {
        // Try to parse buffer head
        match u64::decode_var(&self.buf) {
            Some((len, consumed)) => {
                // Check data length sanity
                if len as usize > MAX_FRAME_LENGTH {
                    return Err(FlipperError::DataTooLarge(len as usize));
                }

                if self.buf.len() >= len as usize + consumed {
                    // Data is ready!
                    self.buf.drain(0..consumed);
                    Ok(Some(self.buf.drain(0..len as usize).collect()))
                } else {
                    Ok(None)
                }
            }
            None => Ok(None),
        }
    }
}

/// Calculate and append header on given frame.
fn build_frame(data: &[u8]) -> Result<Vec<u8>, FlipperError> {
    let mut header = [0u8; 8];

    // Check data length sanity
    if data.len() > MAX_FRAME_LENGTH {
        return Err(FlipperError::DataTooLarge(data.len()));
    }

    let header_len = (data.len() as u64).encode_var(&mut header);

    let mut frame: Vec<u8> = header[..header_len].into();
    frame.extend_from_slice(data);
    Ok(frame)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn try_to_fill_and_drain_u8() {
        let mut packetizer = StreamPacketizer::new();
        packetizer.fill_buffer(&[0x05, 0x01, 0x02, 0x03]);
        assert_eq!(packetizer.poll().unwrap(), None);
        packetizer.fill_buffer(&[0x04, 0x05]);
        assert_eq!(
            packetizer.poll().unwrap(),
            Some(vec![0x01, 0x02, 0x03, 0x04, 0x05])
        );
        assert_eq!(packetizer.poll().unwrap(), None);
    }

    #[test]
    fn check_data_sanitizer() {
        let mut packetizer = StreamPacketizer::new();
        packetizer.fill_buffer(&[0xFE, 0xFF, 0x03, 0x00]); // length 65534
        assert_eq!(packetizer.poll(), Err(FlipperError::DataTooLarge(65534)));
    }

    #[test]
    fn check_basic_build_frame() {
        let res = build_frame(&[0x01, 0x02, 0x03, 0x04, 0x05]).unwrap();
        assert_eq!(res, vec![0x05, 0x01, 0x02, 0x03, 0x04, 0x05]);
    }

    #[test]
    fn check_basic_build_frame_ovf() {
        let large_data: [u8; 65534] = [0; 65534];
        let res = build_frame(&large_data);
        assert_eq!(res, Err(FlipperError::DataTooLarge(65534)))
    }
}
