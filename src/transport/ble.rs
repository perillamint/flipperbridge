/*
 * SPDX-FileCopyrightText: 2022 perillamint
 *
 * SPDX-License-Identifier: MPL-2.0
 *
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

use super::FlipperTransport;
use crate::codec::FlipperCodec;
use crate::consts::{
    BLE_OVERFLOW_CHARACTERISTIC_UUID, BLE_RX_CHARACTERISTIC_UUID, BLE_SERIALSVC_UUID,
    BLE_TX_CHARACTERISTIC_UUID,
};
use crate::error::FlipperError;
use async_stream::stream;
use async_trait::async_trait;
use btleplug::api::{
    Central, Characteristic, Manager as _, Peripheral as _, ScanFilter, WriteType,
};
use btleplug::platform::{Adapter, Manager, Peripheral};
use bytes::{Buf, BufMut, BytesMut};
use futures::stream::{Stream, StreamExt};
use log::{debug, trace, warn};
use pretty_hex::*;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_util::codec::{Decoder, Encoder};

use std::time::Duration;
use tokio::time;

pub struct FlipperScanner {
    bt_adapters: Vec<Adapter>,
    adapter_idx: usize,
}

impl FlipperScanner {
    pub async fn new() -> Result<Self, FlipperError> {
        let manager = Manager::new()
            .await
            .map_err(|e| -> FlipperError { FlipperError::BTFailure(e.to_string()) })?;
        let adapters = manager
            .adapters()
            .await
            .map_err(|e| -> FlipperError { FlipperError::BTAdapterError(e.to_string()) })?;

        if adapters.is_empty() {
            return Err(FlipperError::BTFailure(
                "Adapter does not exist.".to_string(),
            ));
        }

        Ok(Self {
            bt_adapters: adapters,
            adapter_idx: 0,
        })
    }

    /// Fetch adapters present in system
    pub async fn get_adapter_name(&self) -> Result<Vec<String>, FlipperError> {
        let mut ret: Vec<String> = vec![];
        for adapter in self.bt_adapters.iter() {
            let info = adapter
                .adapter_info()
                .await
                .map_err(|e| -> FlipperError { FlipperError::BTAdapterError(e.to_string()) })?;

            ret.push(info);
        }

        Ok(ret)
    }

    /// Set adapter idx
    pub fn set_adapter(&mut self, idx: usize) -> Result<(), FlipperError> {
        if self.bt_adapters.len() > idx {
            self.adapter_idx = idx;
            Ok(())
        } else {
            Err(FlipperError::OutOfBounds)
        }
    }

    pub async fn search_flipper_by_name(&mut self, name: &str) -> Option<Peripheral> {
        let central = &self.bt_adapters[0];
        //central.start_scan(ScanFilter::default()).await.unwrap();
        //time::sleep(Duration::from_secs(2)).await;

        for p in central.peripherals().await.unwrap() {
            if p.properties()
                .await
                .unwrap()
                .unwrap()
                .local_name
                .iter()
                .any(|name| name.contains("Flipper"))
            {
                return Some(p);
            }
        }
        None
    }
}

pub struct BTLEStream {
    flipper: Peripheral,
}

#[derive(Clone)]
pub struct FlipperCharacteristics {
    rx: Characteristic,
    tx: Characteristic,
    ovf: Characteristic,
}

pub struct BTLETransport {
    flipper: Peripheral,
    chars: Option<FlipperCharacteristics>,
    codec: FlipperCodec,
}

impl BTLETransport {
    pub async fn new(flipper: Peripheral) -> Self {
        Self {
            flipper,
            chars: None,
            codec: FlipperCodec::default(),
        }
    }
}

#[async_trait]
impl FlipperTransport for BTLETransport {
    async fn init(&mut self) -> Result<(), FlipperError> {
        self.flipper
            .connect()
            .await
            .map_err(|e| -> FlipperError { FlipperError::BTFailure(e.to_string()) })?;

        self.flipper
            .discover_services()
            .await
            .map_err(|e| -> FlipperError { FlipperError::BTFailure(e.to_string()) })?;

        let chars = self.flipper.characteristics();
        let rx = chars
            .iter()
            .find(|c| c.uuid == *BLE_RX_CHARACTERISTIC_UUID)
            .ok_or(FlipperError::BTNoCharacteristics)?
            .clone();
        let tx = chars
            .iter()
            .find(|c| c.uuid == *BLE_TX_CHARACTERISTIC_UUID)
            .ok_or(FlipperError::BTNoCharacteristics)?
            .clone();
        let ovf = chars
            .iter()
            .find(|c| c.uuid == *BLE_OVERFLOW_CHARACTERISTIC_UUID)
            .ok_or(FlipperError::BTNoCharacteristics)?
            .clone();

        self.flipper.subscribe(&rx).await.unwrap();

        self.chars = Some(FlipperCharacteristics { rx, tx, ovf });

        Ok(())
    }

    async fn read_frame(&mut self) -> Result<Vec<u8>, FlipperError> {
        // Empty the codec first.
        let mut buf = BytesMut::new();
        if let Ok(Some(x)) = self.codec.decode(&mut buf) {
            return Ok(x);
        }

        let chars = self.chars.as_ref().unwrap().clone();
        loop {
            let mut notification = self
                .flipper
                .notifications()
                .await
                .map_err(|e| -> FlipperError { FlipperError::IOFailure(e.to_string()) })?
                .take(1);
            let notif = notification.next().await;

            if notif == None {
                continue;
            }

            let mut buf = BytesMut::new();
            buf.extend_from_slice(&notif.unwrap().value);
            trace!("BTLE RX: {:?}\n", &buf.hex_dump());
            match self.codec.decode(&mut buf) {
                Ok(Some(x)) => return Ok(x),
                Err(e) => return Err(FlipperError::IOFailure(e.to_string())),
                Ok(None) => {} // Data is not ready yet, loop back and wait again.
            }
        }
    }

    async fn write_frame(&mut self, data: &[u8]) -> Result<(), FlipperError> {
        let chars = self.chars.as_ref().unwrap().clone();
        let mut frame: BytesMut = BytesMut::new();
        self.codec.encode(data, &mut frame).unwrap();
        // TODO: Implement chunking and overflow handling
        trace!("BTLE TX: {:?}\n", &frame.hex_dump());

        println!("{:?}\n", self.flipper.read(&chars.ovf).await);
        self.flipper
            .write(&chars.tx, &frame, WriteType::WithoutResponse)
            .await
            .map_err(|e| -> FlipperError { FlipperError::IOFailure(e.to_string()) })?;
        println!("{:?}\n", self.flipper.read(&chars.ovf).await);

        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[tokio::test]
    async fn test() {
        let manager = Manager::new().await.unwrap();
        let central = manager
            .adapters()
            .await
            .expect("Unable to fetch adapter list.");
        println!("{:?}", central);
    }
}
