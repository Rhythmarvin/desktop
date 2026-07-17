//! Transport layer: 5-byte binary frame codec for plugin stdio.
//!
//! Re-exports the protocol crate's frame primitives and adds runtime-aware
//! incremental decoding, single-owner writing, and EOF semantics.

pub mod frame;
pub mod reader;
pub mod writer;

pub use frame::{
    FrameError, FrameType, HEADER_LEN, MAX_PAYLOAD_BYTES, decode_header, encode_frame,
};
pub use reader::FrameReader;
pub use writer::FrameWriter;
