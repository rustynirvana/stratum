use binary_sv2::Error as BinarySv2Error;
use std::fmt::{self, Display, Formatter};

#[derive(Debug)]
/// No NoPairableUpstream((min_v, max_v, all falgs supported))
pub enum Error {
    /// Errors if payload size is too big to fit into a frame.
    BadPayloadSize,
    ExpectedLen32(usize),
    BinarySv2Error(BinarySv2Error),
    /// Errors if a `SendTo::RelaySameMessageSv1` request is made on a SV2-only application.
    CannotRelaySv1Message,
    NoGroupsFound,
    WrongMessageType(u8),
    UnexpectedMessage,
    NoGroupIdOnExtendedChannel,
    /// (`min_v`, `max_v`, all flags supported)
    NoPairableUpstream((u16, u16, u32)),
    /// Error if the hashmap `future_jobs` field in the `GroupChannelJobDispatcher` is empty.
    NoFutureJobs,
    NoDownstreamsConnected,
    PrevHashRequireNonExistentJobId(u32),
    RequestIdNotMapped(u32),
    NoUpstreamsConnected,
    UnimplementedProtocol,
    UnexpectedPoolMessage,
    UnknownRequestId(u32),
    NoMoreExtranonces,
}

impl From<BinarySv2Error> for Error {
    fn from(v: BinarySv2Error) -> Error {
        Error::BinarySv2Error(v)
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        use Error::*;
        match self {
            BadPayloadSize => write!(f, "Payload is too big to fit into the frame"),
            BinarySv2Error(v) => write!(
                f,
                "BinarySv2Error: error in serializing/deserilizing binary format {:?}",
                v
            ),
            CannotRelaySv1Message => {
                write!(
                    f,
                    "Cannot process request: Received SV1 relay request on a SV2-only application"
                )
            }
            ExpectedLen32(l) => write!(f, "Expected length of 32, but received length of {}", l),
            NoGroupsFound => write!(
                f,
                "A channel was attempted to be added to an Upstream, but no groups are specified"
            ),
            WrongMessageType(m) => write!(f, "Wrong message type: {}", m),
            UnexpectedMessage => write!(f, "Error: Unexpected message received"),
            NoGroupIdOnExtendedChannel => write!(f, "Extended channels do not have group IDs"),
            NoPairableUpstream(a) => {
                write!(f, "No pairable upstream node: {:?}", a)
            }
            NoFutureJobs => write!(f, "GroupChannelJobDispatcher does not have any future jobs"),
            NoDownstreamsConnected => write!(f, "NoDownstreamsConnected"),
            PrevHashRequireNonExistentJobId(id) => {
                write!(f, "PrevHashRequireNonExistentJobId {}", id)
            }
            RequestIdNotMapped(id) => write!(f, "RequestIdNotMapped {}", id),
            NoUpstreamsConnected => write!(f, "There are no upstream connected"),
            UnexpectedPoolMessage => write!(f, "Unexpected `PoolMessage` type"),
            UnimplementedProtocol => write!(
                f,
                "TODO: `Protocol` has not been implemented, but should be"
            ),
            UnknownRequestId(id) => write!(
                f,
                "Upstream is answering with a wrong request ID {} or
                DownstreamMiningSelector::on_open_standard_channel_request has not been called
                before relaying open channel request to upstream",
                id
            ),
            NoMoreExtranonces => write!(f, "No more extranonces"),
        }
    }
}
