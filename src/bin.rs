/*
 * SPDX-FileCopyrightText: 2022 perillamint
 *
 * SPDX-License-Identifier: MPL-2.0
 *
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

mod consts;
mod error;
mod transport;

use pretty_hex::*;
use transport::serial::SerialTransport;
use transport::FlipperTransport;

#[tokio::main]
async fn main() {
    env_logger::init();

    let mut transport = SerialTransport::new("/dev/ttyACM0");
    transport.init().await.unwrap();

    transport
        .write_frame(&[0x08, 0x02, 0x82, 0x02, 0x00])
        .await
        .unwrap();

    loop {
        let data = transport.read_frame().await.unwrap();
        println!("{}\n", data.hex_dump());
    }
}
