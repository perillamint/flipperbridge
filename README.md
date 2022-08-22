<!--
SPDX-FileCopyrightText: 2022 perillamint

SPDX-License-Identifier: CC0-1.0
-->

# flipperbridge
Rust implementation of Flipper Zero RPC stream helper library.

### How to use
See Rustdoc. Use `src/bin.rs` as a reference and handy tool for debugging.

### Brief description of FZ RPC frame
Currently, the FZ RPC frame is composed of two parts:

```
+----------------+------------------+
| Frame length   | Frame body       |
| Varint encoded | Protobuf encoded |
+----------------+------------------+
```

and this library provides a handy way to send/receive FZ RPC frames
through the following transports:

* Serial (CDC-ACM, USB)
* Bluetooth LE
