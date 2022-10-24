pub mod events_endpoint;

#[cfg(test)]
pub mod mock_web3_client {
    use std::cell::RefCell;
    use std::fmt::Debug;

    use num256::Uint256;
    use tokio::sync::mpsc::{
        unbounded_channel, UnboundedReceiver, UnboundedSender,
    };
    use tokio::sync::oneshot::Sender;
    use web30::types::Log;

    use super::super::events::signatures::*;
    use super::super::{Error, Result};

    /// Commands we can send to the mock client
    #[derive(Debug)]
    pub enum TestCmd {
        Normal,
        Unresponsive,
        NewHeight(Uint256),
        NewEvent {
            event_type: MockEventType,
            data: Vec<u8>,
            height: u32,
            seen: Sender<()>,
        },
    }

    /// The type of events supported
    #[derive(Debug, PartialEq)]
    pub enum MockEventType {
        TransferToNamada,
        TransferToEthereum,
        ValSetUpdate,
        NewContract,
        UpgradedContract,
        BridgeWhitelist,
    }

    /// A pointer to a mock Web3 client. The
    /// reason is for interior mutability.
    pub struct Web3(RefCell<Web3Client>);

    /// A mock of a web3 api client connected to an ethereum fullnode.
    /// It is not connected to a full node and is fully controllable
    /// via a channel to allow us to mock different behavior for
    /// testing purposes.
    pub struct Web3Client {
        cmd_channel: UnboundedReceiver<TestCmd>,
        active: bool,
        latest_block_height: Uint256,
        events: Vec<(MockEventType, Vec<u8>, u32, Sender<()>)>,
    }

    impl Web3 {
        /// This method is part of the Web3 api we use,
        /// but is not meant to be used in tests
        #[allow(dead_code)]
        pub fn new(_: &str, _: std::time::Duration) -> Self {
            panic!(
                "Method is here for api completeness. It is not meant to be \
                 used in tests."
            )
        }

        /// Return a new client and a separate sender
        /// to send in admin commands
        pub fn setup() -> (UnboundedSender<TestCmd>, Self) {
            // we can only send one command at a time.
            let (cmd_sender, cmd_channel) = unbounded_channel();
            (
                cmd_sender,
                Self(RefCell::new(Web3Client {
                    cmd_channel,
                    active: true,
                    latest_block_height: Default::default(),
                    events: vec![],
                })),
            )
        }

        /// Check and apply new incoming commands
        fn check_cmd_channel(&self) {
            let cmd =
                if let Ok(cmd) = self.0.borrow_mut().cmd_channel.try_recv() {
                    cmd
                } else {
                    return;
                };
            match cmd {
                TestCmd::Normal => self.0.borrow_mut().active = true,
                TestCmd::Unresponsive => self.0.borrow_mut().active = false,
                TestCmd::NewHeight(height) => {
                    self.0.borrow_mut().latest_block_height = height
                }
                TestCmd::NewEvent {
                    event_type: ty,
                    data,
                    height,
                    seen,
                } => self.0.borrow_mut().events.push((ty, data, height, seen)),
            }
        }

        /// Gets the latest block number send in from the
        /// command channel if we have not set the client to
        /// act unresponsive.
        pub async fn eth_block_number(&self) -> Result<Uint256> {
            self.check_cmd_channel();
            Ok(self.0.borrow().latest_block_height.clone())
        }

        /// Gets the events (for the appropriate signature) that
        /// have been added from the command channel unless the
        /// client has not been set to act unresponsive.
        pub async fn check_for_events(
            &self,
            block_to_check: Uint256,
            _: Option<Uint256>,
            _: impl Debug,
            mut events: Vec<&str>,
        ) -> Result<Vec<Log>> {
            self.check_cmd_channel();
            if self.0.borrow().active {
                let ty = match events.remove(0) {
                    TRANSFER_TO_NAMADA_SIG => MockEventType::TransferToNamada,
                    TRANSFER_TO_ETHEREUM_SIG => {
                        MockEventType::TransferToEthereum
                    }
                    VALIDATOR_SET_UPDATE_SIG => MockEventType::ValSetUpdate,
                    NEW_CONTRACT_SIG => MockEventType::NewContract,
                    UPGRADED_CONTRACT_SIG => MockEventType::UpgradedContract,
                    UPDATE_BRIDGE_WHITELIST_SIG => {
                        MockEventType::BridgeWhitelist
                    }
                    _ => return Ok(vec![]),
                };
                let mut logs = vec![];
                let mut events = vec![];
                let mut client = self.0.borrow_mut();
                std::mem::swap(&mut client.events, &mut events);
                for (event_ty, data, height, seen) in events.into_iter() {
                    if event_ty == ty && block_to_check >= Uint256::from(height)
                    {
                        seen.send(()).unwrap();
                        logs.push(Log {
                            data: data.into(),
                            ..Default::default()
                        });
                    } else {
                        client.events.push((event_ty, data, height, seen));
                    }
                }
                Ok(logs)
            } else {
                Err(Error::Runtime("Uh oh, I'm not responding".into()))
            }
        }
    }
}