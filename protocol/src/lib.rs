pub mod ptr;
pub mod socket;

use serde::{Deserialize, Serialize};

pub const SERVER_PROTOCOL_VERSION: i32 = 1;

pub const WINE_ARCH: &'static str = "WINE_ARCH";
pub const WINE_SERVER_SOCKET: &'static str = "WINE_SERVER_SOCKET";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WineRequest {
    Ping,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WineReply {
    Pong,
}

/// NT-style timeout, in 100ns units, negative means relative timeout
#[derive(Copy, Clone, Default)]
pub struct Timeout(pub i64);

impl Timeout {
    pub const INFINITE: Timeout = Timeout(0x7fffffffi64 << 32 | 0xffffffff);
    pub const TICKS_PER_SECOND: Timeout = Timeout(10000000);
    pub const TICKS_1601_TO_1970: Timeout =
        Timeout(86400 * (369 * 365 + 89) * Timeout::TICKS_PER_SECOND.0);
}

/// absolute timeout, negative means that monotonic clock is used
#[derive(Copy, Clone)]
pub struct AbsoluteTime(pub i64);
