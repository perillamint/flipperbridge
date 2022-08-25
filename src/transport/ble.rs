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
use crate::codec::FlipperCodec;
use crate::consts::{
    BLE_OVERFLOW_CHARACTERISTIC_UUID, BLE_RX_CHARACTERISTIC_UUID, BLE_TX_CHARACTERISTIC_UUID,
};
use crate::error::FlipperError;
use async_lock::RwLock;
use async_trait::async_trait;
use btleplug::api::{
    Central, Characteristic, Manager as _, Peripheral as _, ValueNotification, WriteType,
};
use btleplug::platform::{Adapter, Manager, Peripheral};
use bytes::BytesMut;
use futures::stream::{Stream, StreamExt};
use log::trace;
use pretty_hex::*;
use std::pin::Pin;
use std::sync::Arc;
use tokio_util::codec::{Decoder, Encoder};

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

    pub async fn search_flipper_by_name(&mut self, flipper_name: &str) -> Option<Peripheral> {
        let central = &self.bt_adapters[self.adapter_idx];

        for p in central.peripherals().await.unwrap() {
            if p.properties()
                .await
                .unwrap()
                .unwrap()
                .local_name
                .iter()
                .any(|name| name.contains(flipper_name))
            {
                return Some(p);
            }
        }
        None
    }
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
    notification_stream: Option<Pin<Box<dyn Stream<Item = ValueNotification> + Send>>>,
}

impl BTLETransport {
    pub async fn new(flipper: Peripheral) -> Self {
        Self {
            flipper,
            chars: None,
            codec: FlipperCodec::default(),
            notification_stream: None,
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

        self.notification_stream = Some(
            self.flipper
                .notifications()
                .await
                .map_err(|e| -> FlipperError { FlipperError::IOFailure(e.to_string()) })?,
        );

        Ok(())
    }

    async fn split_stream(self) -> (Box<dyn FlipperFrameReceiver>, Box<dyn FlipperFrameSender>) {
        let sharable_flipper = Arc::new(RwLock::new(self.flipper));
        let chars = self.chars.expect("Not initialized!");
        (
            Box::new(BTLEFrameReceiver::new(sharable_flipper.clone(), chars.rx)),
            Box::new(BTLEFrameSender::new(sharable_flipper, chars.tx, chars.ovf)),
        )
    }
}

#[async_trait]
impl FlipperFrameReceiver for BTLETransport {
    async fn read_frame(&mut self) -> Result<Vec<u8>, FlipperError> {
        // Empty the codec first.
        let mut buf = BytesMut::new();
        if let Ok(Some(x)) = self.codec.decode(&mut buf) {
            return Ok(x);
        }

        //let chars = self.chars.as_ref().unwrap().clone();
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
}

#[async_trait]
impl FlipperFrameSender for BTLETransport {
    async fn write_frame(&mut self, data: &[u8]) -> Result<(), FlipperError> {
        let chars = self.chars.as_ref().unwrap().clone();
        let mut frame: BytesMut = BytesMut::new();
        self.codec.encode(data, &mut frame).unwrap();
        // TODO: Implement chunking and overflow handling
        trace!("BTLE TX: {:?}\n", &frame.hex_dump());
        let bufsz = u32::from_be_bytes(
            self.flipper
                .read(&chars.ovf)
                .await
                .unwrap()
                .try_into()
                .unwrap(),
        );
        println!("remaining buffer: {:?}\n", bufsz);
        self.flipper
            .write(&chars.tx, &frame, WriteType::WithoutResponse)
            .await
            .map_err(|e| -> FlipperError { FlipperError::IOFailure(e.to_string()) })?;
        println!("{:?}\n", self.flipper.read(&chars.ovf).await);

        Ok(())
    }
}

pub struct BTLEFrameSender {
    tx_characteristic: Characteristic,
    ovf_characteristic: Characteristic,
    flipper: Arc<RwLock<Peripheral>>,
    codec: FlipperCodec,
}

impl BTLEFrameSender {
    fn new(
        flipper: Arc<RwLock<Peripheral>>,
        tx_chr: Characteristic,
        ovf_chr: Characteristic,
    ) -> Self {
        Self {
            flipper,
            tx_characteristic: tx_chr,
            ovf_characteristic: ovf_chr,
            codec: FlipperCodec::default(),
        }
    }
}

#[async_trait]
impl FlipperFrameSender for BTLEFrameSender {
    async fn write_frame(&mut self, data: &[u8]) -> Result<(), FlipperError> {
        let mut frame: BytesMut = BytesMut::new();
        self.codec.encode(data, &mut frame).unwrap();
        // TODO: Implement chunking and overflow handling
        trace!("BTLE TX: {:?}\n", &frame.hex_dump());
        let bufsz = u32::from_be_bytes(
            self.flipper
                .read()
                .await
                .read(&self.ovf_characteristic)
                .await
                .unwrap()
                .try_into()
                .unwrap(),
        );
        println!("remaining buffer: {:?}\n", bufsz);
        self.flipper
            .read()
            .await
            .write(&self.tx_characteristic, &frame, WriteType::WithoutResponse)
            .await
            .map_err(|e| -> FlipperError { FlipperError::IOFailure(e.to_string()) })?;

        Ok(())
    }
}

pub struct BTLEFrameReceiver {
    _rx_characteristic: Characteristic,
    flipper: Arc<RwLock<Peripheral>>,
    codec: FlipperCodec,
}

impl BTLEFrameReceiver {
    fn new(flipper: Arc<RwLock<Peripheral>>, rx_chr: Characteristic) -> Self {
        Self {
            flipper,
            _rx_characteristic: rx_chr,
            codec: FlipperCodec::default(),
        }
    }
}

#[async_trait]
impl FlipperFrameReceiver for BTLEFrameReceiver {
    async fn read_frame(&mut self) -> Result<Vec<u8>, FlipperError> {
        // Empty the codec first.
        let mut buf = BytesMut::new();
        if let Ok(Some(x)) = self.codec.decode(&mut buf) {
            return Ok(x);
        }

        loop {
            let mut notification = self
                .flipper
                .read()
                .await
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
