use async_channel::bounded;
use codec_sv2::{
    noise_sv2::formats::{EncodedEd25519PublicKey, EncodedEd25519SecretKey},
    StandardEitherFrame, StandardSv2Frame,
};
use roles_logic_sv2::{
    bitcoin::{secp256k1::Secp256k1, Network, PrivateKey, PublicKey},
    parsers::PoolMessages,
};
use serde::Deserialize;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

mod lib;

use lib::{mining_pool::Pool, template_receiver::TemplateRx};
use logging::{log_given_level, log_info, log_internal, Level, Logger, Record};

pub type Message = PoolMessages<'static>;
pub type StdFrame = StandardSv2Frame<Message>;
pub type EitherFrame = StandardEitherFrame<Message>;

const HOM_GROUP_ID: u32 = u32::MAX;

const PRIVATE_KEY_BTC: [u8; 32] = [34; 32];
const NETWORK: Network = Network::Testnet;

const BLOCK_REWARD: u64 = 625_000_000_000;

fn new_pub_key() -> PublicKey {
    let priv_k = PrivateKey::from_slice(&PRIVATE_KEY_BTC, NETWORK).unwrap();
    let secp = Secp256k1::default();
    PublicKey::from_private_key(&secp, &priv_k)
}

#[derive(Debug, Deserialize)]
pub struct Configuration {
    pub listen_address: String,
    pub tp_address: String,
    pub authority_public_key: EncodedEd25519PublicKey,
    pub authority_secret_key: EncodedEd25519SecretKey,
    pub cert_validity_sec: u64,
}

mod args {
    use std::path::PathBuf;

    #[derive(Debug)]
    pub struct Args {
        pub config_path: PathBuf,
    }

    enum ArgsState {
        Next,
        ExpectPath,
        Done,
    }

    enum ArgsResult {
        Config(PathBuf),
        None,
        Help(String),
    }

    impl Args {
        const DEFAULT_CONFIG_PATH: &'static str = "pool-config.toml";

        pub fn from_args() -> Result<Self, String> {
            let cli_args = std::env::args();

            let config_path = cli_args
                .scan(ArgsState::Next, |state, item| {
                    match std::mem::replace(state, ArgsState::Done) {
                        ArgsState::Next => match item.as_str() {
                            "-c" | "--config" => {
                                *state = ArgsState::ExpectPath;
                                Some(ArgsResult::None)
                            }
                            "-h" | "--help" => Some(ArgsResult::Help(format!(
                                "Usage: -h/--help, -c/--config <path|default {}>",
                                Self::DEFAULT_CONFIG_PATH
                            ))),
                            _ => {
                                *state = ArgsState::Next;

                                Some(ArgsResult::None)
                            }
                        },
                        ArgsState::ExpectPath => Some(ArgsResult::Config(PathBuf::from(item))),
                        ArgsState::Done => None,
                    }
                })
                .last();
            let config_path = match config_path {
                Some(ArgsResult::Config(p)) => p,
                Some(ArgsResult::Help(h)) => return Err(h),
                _ => PathBuf::from(Self::DEFAULT_CONFIG_PATH),
            };
            Ok(Self { config_path })
        }
    }
}

#[derive(Debug)]
struct TrackingLogger {
    /// (module, message) -> count
    pub lines: Mutex<HashMap<(String, String), usize>>,
}

impl Logger for TrackingLogger {
    fn log(&self, record: &Record) {
        *self
            .lines
            .lock()
            .unwrap()
            .entry((record.module_path.to_string(), format!("{}", record.args)))
            .or_insert(0) += 1;
        println!(
            "{:<5} [{} : {}, {}] {}",
            record.level.to_string(),
            record.module_path,
            record.file,
            record.line,
            record.args
        );
    }
}

#[tokio::main]
async fn main() {
    let args = match args::Args::from_args() {
        Ok(cfg) => cfg,
        Err(help) => {
            println!("{}", help);
            return;
        }
    };
    let config_file = std::fs::read_to_string(args.config_path).expect("TODO: Error handling");
    let config = match toml::from_str::<Configuration>(&config_file) {
        Ok(cfg) => cfg,
        Err(e) => {
            println!("Failed to parse config file: {}", e);
            return;
        }
    };

    let logger = TrackingLogger {
        lines: Mutex::new(HashMap::new()),
    };

    log_info!(logger, "Starting pool - WITH LOGGER");


    let (s_new_t, r_new_t) = bounded(10);
    let (s_prev_hash, r_prev_hash) = bounded(10);
    let (s_solution, r_solution) = bounded(10);
    println!("POOL INTITIALIZING ");

    let logger = Arc::new(logger);

    TemplateRx::connect(
        logger.clone(),
        config.tp_address.parse().unwrap(),
        s_new_t,
        s_prev_hash,
        r_solution,
    )
    .await;
    println!("POOL INITIALIZED");

    Pool::start(logger.clone(),
                        config, r_new_t, r_prev_hash, s_solution).await;
}
