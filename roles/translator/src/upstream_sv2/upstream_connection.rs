use super::{EitherFrame, StdFrame};
use crate::ProxyResult;
use async_channel::{Receiver, Sender};

/// Handles the sending and receiving of messages to and from an SV2 Upstream role (most typically
/// a SV2 Pool server).
/// On upstream, we have a sv2connection, so we use the connection from network helpers
/// use network_helpers::Connection;
/// this does the dirty work of reading byte by byte in the socket and puts them in a complete
/// Sv2Messages frame and when the message is ready then sends to our Upstream
/// sender_incoming + receiver_outgoing are in network_helpers::Connection
#[derive(Debug, Clone)]
pub struct UpstreamConnection {
    /// Receives messages from the SV2 Upstream role
    pub receiver: Receiver<EitherFrame>,
    /// Sends messages to the SV2 Upstream role
    pub sender: Sender<EitherFrame>,
}

impl UpstreamConnection {
    /// Send a SV2 message to the Upstream role
    pub async fn send(&mut self, sv2_frame: StdFrame) -> ProxyResult<()> {
        println!("TU SEND TO UPSTREAM: {:?}", &sv2_frame);
        let either_frame = sv2_frame.into();
        self.sender
            .send(either_frame)
            .await
            .expect("Error sending `EitherFrame` to the Upstream role");
        Ok(())
    }
}
