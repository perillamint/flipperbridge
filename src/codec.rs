/*
 * SPDX-FileCopyrightText: 2022 perillamint
 *
 * SPDX-License-Identifier: MPL-2.0
 *
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

use crate::consts::MAX_FRAME_LENGTH;
use bytes::{Buf, BufMut, BytesMut};
use integer_encoding::VarInt;
use std::io::{Error, ErrorKind, Result};
use tokio_util::codec::{Decoder, Encoder};

#[derive(Default)]
pub(crate) struct FlipperCodec {
    buf: Vec<u8>,
}

impl Decoder for FlipperCodec {
    type Item = Vec<u8>;
    type Error = Error;

    fn decode(&mut self, buf: &mut BytesMut) -> Result<Option<Vec<u8>>> {
        self.buf.extend_from_slice(buf);
        buf.advance(buf.len());
        match u64::decode_var(&self.buf) {
            Some((len, consumed)) => {
                // Check data length sanity
                if len as usize > MAX_FRAME_LENGTH {
                    return Err(Error::new(
                        ErrorKind::InvalidData,
                        "Data too big!".to_string(),
                    ));
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

impl Encoder<&[u8]> for FlipperCodec {
    type Error = Error;

    fn encode(&mut self, data: &[u8], buf: &mut BytesMut) -> Result<()> {
        let mut header = [0u8; 8];

        // Check data length sanity
        if data.len() > MAX_FRAME_LENGTH {
            return Err(Error::new(
                ErrorKind::InvalidData,
                "Data too big!".to_string(),
            ));
        }

        let header_len = (data.len() as u64).encode_var(&mut header);
        buf.put_slice(&header[..header_len]);
        buf.put_slice(data);
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn try_to_fill_and_drain_u8() {
        let mut codec = FlipperCodec::default();
        let mut buf = BytesMut::new();
        buf.put_slice(&[0x05, 0x01, 0x02, 0x03]);

        let res_1 = codec.decode(&mut buf).unwrap();
        assert_eq!(res_1, None);

        buf.put_slice(&[0x04, 0x05]);
        let res_2 = codec.decode(&mut buf).unwrap();
        assert_eq!(res_2, Some(vec![0x01, 0x02, 0x03, 0x04, 0x05]));
    }

    #[test]
    fn check_data_sanitizer() {
        let mut codec = FlipperCodec::default();
        let mut buf = BytesMut::new();
        buf.put_slice(&[0xFE, 0xFF, 0x03, 0x00]); // length 65534
        if let Err(err) = codec.decode(&mut buf) {
            assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
        } else {
            panic!("It should error out!");
        }
    }

    #[test]
    fn check_basic_build_frame() {
        let mut codec = FlipperCodec::default();
        let mut buf: BytesMut = BytesMut::new();
        codec
            .encode(&[0x01, 0x02, 0x03, 0x04, 0x05], &mut buf)
            .unwrap();
        assert_eq!(buf, vec![0x05, 0x01, 0x02, 0x03, 0x04, 0x05]);
    }

    #[test]
    fn check_basic_build_frame_ovf() {
        let mut codec = FlipperCodec::default();
        let mut buf: BytesMut = BytesMut::new();
        let large_data: [u8; 65534] = [0; 65534];
        if let Err(err) = codec.encode(&large_data, &mut buf) {
            assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
        } else {
            panic!("It should error out!");
        }
    }
}
