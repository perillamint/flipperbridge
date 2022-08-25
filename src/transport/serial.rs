/*
 * SPDX-FileCopyrightText: 2022 perillamint
 *
 * SPDX-License-Identifier: MPL-2.0
 *
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

use super::{FlipperFrameReceiver, FlipperFrameSender, FlipperTransport};
use crate::consts::PROMPT_PATTERN;
use crate::error::FlipperError;
use async_trait::async_trait;
use futures::sink::SinkExt;
use futures::stream::StreamExt;
use log::{debug, trace};
use tokio::io::split;
use tokio::io::{AsyncReadExt, AsyncWriteExt, ReadHalf, WriteHalf};
use tokio_serial::{self, SerialPortBuilderExt, SerialStream};

use crate::codec::FlipperCodec;
use tokio_util::codec::{Framed, FramedRead, FramedWrite};

use pretty_hex::*;

const FLIPPER_BAUD: u32 = 115200;

/// Find subsequence in u8 slice.
/// Code from https://stackoverflow.com/questions/35901547/how-can-i-find-a-subsequence-in-a-u8-slice
fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

/// Serial transport for Flipper Zero
pub struct SerialTransport {
    tty: String,
    framed: Option<Framed<SerialStream, FlipperCodec>>,
}

impl SerialTransport {
    /// Create SerialTransport using tty path.
    /// for example, "/dev/ttyACM0" or "COM1"
    pub fn new(tty: &str) -> Self {
        Self {
            tty: tty.to_string(),
            framed: None,
        }
    }

    /// Write raw bytes async-y to the stream.
    /// Internal use only.
    async fn write_raw(port: &mut SerialStream, data: &[u8]) -> Result<(), FlipperError> {
        trace!("Serial Write - {}", data.hex_dump());
        // Write to the serial port
        let mut pos = 0;
        while pos < data.len() {
            let n = match port.write(&data[pos..]).await {
                Ok(x) => x,
                Err(e) => return Err(FlipperError::IOFailure(e.to_string())),
            };
            pos += n;
        }

        port.flush().await.unwrap();

        Ok(())
    }

    /// Drain Serial stream until specific pattern.
    /// Like, draining until FZShell prompt. Internal use only.
    async fn drain_until_pattern(
        port: &mut SerialStream,
        pattern: &[u8],
    ) -> Result<(), FlipperError> {
        let mut patternbuf: Vec<u8> = vec![];
        let mut buf = [0u8; 1024];

        // TODO: Implement timeout.
        loop {
            let readsz = port.read(&mut buf).await.unwrap();

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
    /// Initialize and prepare serial stream for FZ RPC communication.
    /// Must be called before start sending / receiving RPC command frames.
    async fn init(&mut self) -> Result<(), FlipperError> {
        let mut port = tokio_serial::new(&self.tty, FLIPPER_BAUD)
            .open_native_async()
            .unwrap();
        Self::drain_until_pattern(&mut port, &PROMPT_PATTERN).await?;
        debug!("FZShell detected. Running start_rpc_session\n");

        Self::write_raw(&mut port, "start_rpc_session\r".as_bytes()).await?;
        Self::drain_until_pattern(&mut port, "start_rpc_session\r\n".as_bytes()).await?;
        debug!("Got command response.\n");
        self.framed = Some(Framed::new(port, FlipperCodec::default()));

        Ok(())
    }

    fn split_stream(self) -> (Box<dyn FlipperFrameReceiver>, Box<dyn FlipperFrameSender>) {
        let (rx, tx) = split(self.framed.unwrap().into_inner());

        (
            Box::new(SerialFrameReceiver::new(rx)),
            Box::new(SerialFrameSender::new(tx)),
        )
    }
}

#[async_trait]
impl FlipperFrameReceiver for SerialTransport {
    /// Read variable size FZ RPC frame.
    async fn read_frame(&mut self) -> Result<Vec<u8>, FlipperError> {
        loop {
            match self.framed.as_mut().unwrap().next().await {
                None => {}
                Some(x) => return Ok(x.unwrap()),
            };
        }
    }
}

#[async_trait]
impl FlipperFrameSender for SerialTransport {
    /// Write(send) FZ RPC frame. Frame header will be automatically calculated and appended.
    async fn write_frame(&mut self, data: &[u8]) -> Result<(), FlipperError> {
        match self.framed.as_mut().unwrap().send(data).await {
            Ok(_) => Ok(()),
            Err(e) => Err(FlipperError::IOFailure(e.to_string())),
        }
    }
}

struct SerialFrameSender {
    framed: FramedWrite<WriteHalf<SerialStream>, FlipperCodec>,
}

impl SerialFrameSender {
    fn new(write_stream: WriteHalf<SerialStream>) -> Self {
        Self {
            framed: FramedWrite::new(write_stream, FlipperCodec::default()),
        }
    }
}

#[async_trait]
impl FlipperFrameSender for SerialFrameSender {
    /// Write(send) FZ RPC frame. Frame header will be automatically calculated and appended.
    async fn write_frame(&mut self, data: &[u8]) -> Result<(), FlipperError> {
        match self.framed.send(data).await {
            Ok(_) => Ok(()),
            Err(e) => Err(FlipperError::IOFailure(e.to_string())),
        }
    }
}

struct SerialFrameReceiver {
    framed: FramedRead<ReadHalf<SerialStream>, FlipperCodec>,
}

impl SerialFrameReceiver {
    fn new(read_stream: ReadHalf<SerialStream>) -> Self {
        Self {
            framed: FramedRead::new(read_stream, FlipperCodec::default()),
        }
    }
}

#[async_trait]
impl FlipperFrameReceiver for SerialFrameReceiver {
    /// Read variable size FZ RPC frame.
    async fn read_frame(&mut self) -> Result<Vec<u8>, FlipperError> {
        loop {
            match self.framed.next().await {
                None => {}
                Some(x) => return Ok(x.unwrap()),
            };
        }
    }
}
