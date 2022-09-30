#[cfg(not(feature = "with_serde"))]
use alloc::vec::Vec;
#[cfg(not(feature = "with_serde"))]
use binary_sv2::binary_codec_sv2;
use binary_sv2::{Deserialize, Serialize, Str0255, U256};
use core::convert::TryInto;

/// # UpdateChannel (Client -> Server)
///
/// Client notifies the server about changes on the specified channel. If a client performs
/// device/connection aggregation (i.e. it is a proxy), it MUST send this message when downstream
/// channels change. This update can be debounced so that it is not sent more often than once in a
/// second (for a very busy proxy).
///
/// This message is an extended channel only message. Using it in other kind if channels should
/// raise an error
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct UpdateChannel<'decoder> {
    /// Channel identification.
    pub channel_id: u32,
    /// See Open*Channel for details.
    pub nominal_hash_rate: f32,
    /// Maximum target is changed by server by sending SetTarget. This
    /// field is understood as device’s request. There can be some delay
    /// between UpdateChannel and corresponding SetTarget messages,
    /// based on new job readiness on the server.
    ///
    /// When maximum_target is smaller than currently used maximum target for the channel,
    /// upstream node MUST reflect the client’s request (and send appropriate SetTarget message).
    #[cfg_attr(feature = "with_serde", serde(borrow))]
    pub maximum_target: U256<'decoder>,
}

/// # Update.Error (Server -> Client)
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct UpdateChannelError<'decoder> {
    /// Channel identification.
    pub channel_id: u32,
    /// Human-readable error code(s).
    /// Possible error codes:
    /// * ‘max-target-out-of-range’
    /// * ‘invalid-channel-id’
    #[cfg_attr(feature = "with_serde", serde(borrow))]
    pub error_code: Str0255<'decoder>,
}
