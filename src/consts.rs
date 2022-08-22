/*
 * SPDX-FileCopyrightText: 2022 perillamint
 *
 * SPDX-License-Identifier: MPL-2.0
 *
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

use lazy_static::lazy_static;
use uuid::Uuid;

/// Flipper Zero max frame length.
/// Value from flipper firmware applications/rpc/rpc.h
pub const MAX_FRAME_LENGTH: usize = 1536;
/// Flipper zero prompt pattern in u8 slice.
/// Human readable representation: '\n>: '
pub const PROMPT_PATTERN: [u8; 4] = [0x0a, 0x3e, 0x3a, 0x20];

lazy_static! {
    /// BLE GATT characteristic UUIDs are originated from
    /// https://github.com/flipperdevices/Flipper-Android-App/blob/master/components/bridge/api/src/main/java/com/flipperdevices/bridge/api/utils/Constants.kt
    /// Flipper Zero BLE Serial Service UUID
    pub static ref BLE_SERIALSVC_UUID: Uuid = Uuid::parse_str("8fe5b3d5-2e7f-4a98-2a48-7acc60fe0000").unwrap();
    /// Flipper Zero Read characteristic UUID
    pub static ref BLE_RX_CHARACTERISTIC_UUID: Uuid = Uuid::parse_str("19ed82ae-ed21-4c9d-4145-228e61fe0000").unwrap();
    /// Flipper Zero Write characteristic UUID
    pub static ref BLE_TX_CHARACTERISTIC_UUID: Uuid = Uuid::parse_str("19ed82ae-ed21-4c9d-4145-228e62fe0000").unwrap();
    /// Flipper Zero Overflow characteristic UUID
    pub static ref BLE_OVERFLOW_CHARACTERISTIC_UUID: Uuid = Uuid::parse_str("19ed82ae-ed21-4c9d-4145-228e63fe0000").unwrap();
}
