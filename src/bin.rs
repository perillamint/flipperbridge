/*
 * SPDX-FileCopyrightText: 2022 perillamint
 *
 * SPDX-License-Identifier: MPL-2.0
 *
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

mod codec;
mod consts;
mod error;
mod transport;

use async_lock::RwLock;
use pretty_hex::*;
use std::sync::Arc;
use transport::ble::{BTLETransport, FlipperScanner};
use transport::serial::SerialTransport;
use transport::FlipperTransport;

use clap::Parser;

#[macro_use]
extern crate lazy_static;

#[derive(clap::Parser)]
#[clap(about, version, author)]
struct Args {
    #[clap(long, short = 't', value_name = "TRANSPORT`")]
    transport: String,
}

lazy_static! {
    static ref ARGS: Args = Args::parse();
}

#[tokio::main]
async fn main() {
    env_logger::init();
    match ARGS.transport.as_str() {
        "ble" => {
            btle_example().await;
        }
        "serial" => {
            serial_example().await;
        }
        _ => {
            println!("Require transport type. Use --help for more information.");
        }
    }
}

async fn serial_example() {
    let mut transport = SerialTransport::new("/dev/ttyACM0");
    transport.init().await.unwrap();

    let (mut receiver, mut sender) = transport.into_channel();
    let recv_thread = tokio::spawn(async move {
        loop {
            let data = receiver.read_frame().await.unwrap();
            println!("{:?}\n", data.hex_dump());
        }
    });

    sender.write_frame(&[0x08, 0x02, 0x82, 0x02, 0x00]).await;

    futures::join!((recv_thread));
}

async fn btle_example() {
    let mut scanner = FlipperScanner::new().await.unwrap();
    let adapters = scanner.get_adapter_name().await.unwrap();
    println!("{:?}", adapters);
    // Just use adapter zero.
    scanner.set_adapter(0).unwrap();
    // search it.
    let flip = scanner.search_flipper_by_name("Flipper ").await.unwrap();
    println!("{:?}", flip);

    let mut transport = BTLETransport::new(flip).await;
    transport.init().await.unwrap();

    let (mut receiver, mut sender) = transport.into_channel();
    let recv_thread = tokio::spawn(async move {
        loop {
            let data = receiver.read_frame().await.unwrap();
            println!("{:?}\n", data.hex_dump());
        }
    });

    sender.write_frame(&[0x08, 0x02, 0x82, 0x02, 0x00]).await;

    futures::join!((recv_thread));
}
