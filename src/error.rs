/*
 * SPDX-FileCopyrightText: 2022 perillamint
 *
 * SPDX-License-Identifier: MPL-2.0
 *
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum FlipperError {
    #[error("Failed to fetch adapter list: {0}")]
    BTAdapterError(String),
    #[error("Generic BT error: {0}")]
    BTFailure(String),
    #[error("BT characteristics does not exist. Maybe invalid device?")]
    BTNoCharacteristics,
    #[error("Failed to do I/O: {0}")]
    IOFailure(String),
    #[error("Data too large to process: {0}")]
    DataTooLarge(usize),
    #[error("Index out of bounds.")]
    OutOfBounds,
    #[error("Unknown internal error. BAD!")]
    Unknown,
}
