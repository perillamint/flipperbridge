/*
 * SPDX-FileCopyrightText: 2022 perillamint
 *
 * SPDX-License-Identifier: MPL-2.0
 *
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

use super::{build_frame, FlipperTransport, StreamPacketizer};
use crate::consts::PROMPT_PATTERN;
use crate::error::FlipperError;
use async_trait::async_trait;
use log::{debug, trace, warn};
use tokio::io::{AsyncReadExt, AsyncWriteExt, ErrorKind};
use tokio_serial::{self, SerialPortBuilderExt, SerialStream};

use pretty_hex::*;

const FLIPPER_BAUD: u32 = 115200;

// Code from https://stackoverflow.com/questions/35901547/how-can-i-find-a-subsequence-in-a-u8-slice
fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

pub struct SerialTransport {
    port: SerialStream,
    packetizer: StreamPacketizer,
}

impl SerialTransport {
    pub fn new(tty: &str) -> Self {
        let port = tokio_serial::new(tty, FLIPPER_BAUD)
            .open_native_async()
            .unwrap();
        Self {
            port,
            packetizer: StreamPacketizer::new(),
        }
    }

    async fn write_raw(&mut self, data: &[u8]) -> Result<(), FlipperError> {
        trace!("Serial Write - {}", data.hex_dump());
        // Write to the serial port
        let mut pos = 0;
        while pos < data.len() {
            let n = match self.port.write(&data[pos..]).await {
                Ok(x) => x,
                Err(e) => return Err(FlipperError::IOFailure(e.to_string())),
            };
            pos += n;
        }

        self.port.flush().await.unwrap();

        Ok(())
    }

    async fn drain_until_pattern(&mut self, pattern: &[u8]) -> Result<(), FlipperError> {
        let mut patternbuf: Vec<u8> = vec![];
        let mut buf = [0u8; 1024];

        // TODO: Implement timeout.
        loop {
            let readsz = self.port.read(&mut buf).await.unwrap();

            trace!("Serial Read - {}", buf[0..readsz].hex_dump());
            patternbuf.extend_from_slice(&buf[0..readsz]);

            if patternbuf.len() > 32 {
                patternbuf.drain(0..(patternbuf.len() - 32));
            }

            match find_subsequence(&patternbuf, pattern) {
                Some(_) => return Ok(()),
                None => {}
            }
        }
    }
}

#[async_trait]
impl FlipperTransport for SerialTransport {
    async fn init(&mut self) -> Result<(), FlipperError> {
        self.drain_until_pattern(&PROMPT_PATTERN).await?;
        debug!("FZShell detected. Running start_rpc_session\n");

        self.write_raw("start_rpc_session\r".as_bytes()).await?;
        self.drain_until_pattern("start_rpc_session\r\n".as_bytes())
            .await?;
        debug!("Got command response.\n");
        Ok(())
    }

    async fn read_frame(&mut self) -> Result<Vec<u8>, FlipperError> {
        let mut buf = [0u8; 1024];
        loop {
            let readsz = self.port.read(&mut buf).await.unwrap();
            debug!("Current pkt: {:?}\n", &buf[0..readsz]);
            self.packetizer.fill_buffer(&buf[0..readsz]);
            match self.packetizer.poll() {
                Ok(Some(x)) => return Ok(x),
                Err(e) => return Err(e),
                Ok(None) => {} // Data is not ready yet, loop back and wait again.
            }
        }
    }

    async fn write_frame(&mut self, data: &[u8]) -> Result<(), FlipperError> {
        let frame = build_frame(data)?;
        self.write_raw(&frame).await
    }
}
