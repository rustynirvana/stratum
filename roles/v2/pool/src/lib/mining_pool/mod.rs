use codec_sv2::{HandshakeRole, Responder};
use network_helpers::noise_connection_tokio::Connection;
use tokio::{net::TcpListener, task};

use crate::{Configuration, EitherFrame, StdFrame};
use async_channel::{Receiver, Sender};
use binary_sv2::{B064K, U256};
use bitcoin::{
    blockdata::block::BlockHeader,
    hash_types::BlockHash,
    hashes::{sha256d::Hash, Hash as Hash_},
    util::uint::Uint256,
    TxMerkleNode,
};
use codec_sv2::Frame;
use logging_sv2::{log_given_level, log_info, log_internal, Level, Logger, Record};

use roles_logic_sv2::{
    common_properties::{CommonDownstreamData, IsDownstream, IsMiningDownstream},
    errors::Error,
    handlers::mining::{ParseDownstreamMiningMessages, SendTo},
    job_creator::JobsCreators,
    mining_sv2::{ExtendedExtranonce, NewExtendedMiningJob, SetNewPrevHash as NewPrevHash},
    parsers::{Mining, PoolMessages},
    routing_logic::MiningRoutingLogic,
    template_distribution_sv2::{NewTemplate, SetNewPrevHash, SubmitSolution},
    utils::{merkle_root_from_path, Id, Mutex},
};
use std::{collections::HashMap, convert::TryInto, sync::Arc};
use std::fmt::Debug;
use std::ops::Deref;

pub fn u256_to_block_hash(v: U256<'static>) -> BlockHash {
    let hash: [u8; 32] = v.to_vec().try_into().unwrap();
    let hash = Hash::from_inner(hash);
    BlockHash::from_hash(hash)
}

pub mod setup_connection;
use setup_connection::SetupConnectionHandler;

pub mod message_handler;

#[derive(Debug, Clone)]
struct PartialJob {
    target: Uint256,
    extranonce: Vec<u8>,
}

impl PartialJob {
    pub fn to_complete_standard_job(
        &self,
        new_ext_job: &NewExtendedMiningJob<'static>,
        nbits: u32,
        prev_hash: BlockHash,
        template_id: u64,
    ) -> CompleteJob {
        let merkle_root: [u8; 32] = merkle_root_from_path(
            &(new_ext_job.coinbase_tx_prefix.to_vec()[..]),
            &(new_ext_job.coinbase_tx_suffix.to_vec()[..]),
            &(self.extranonce[..]),
            &(new_ext_job.merkle_path.inner_as_ref()[..]),
        )
        .unwrap()
        .try_into()
        .unwrap();
        let merkle_root = Hash::from_inner(merkle_root);
        let merkle_root = TxMerkleNode::from_hash(merkle_root);
        CompleteJob {
            target: self.target,
            nbits,
            prev_hash,
            new_shares_sum: 0,
            coinbase_tx_prefix: new_ext_job.coinbase_tx_prefix.to_vec(),
            coinbase_tx_suffix: new_ext_job.coinbase_tx_suffix.to_vec(),
            merkle_path: new_ext_job.merkle_path.to_vec(),
            extranonce: self.extranonce.clone(),
            merkle_root,
            template_id,
        }
    }
}
#[derive(Debug, Clone)]
struct CompleteJob {
    template_id: u64,
    target: Uint256,
    nbits: u32,
    prev_hash: BlockHash,
    new_shares_sum: u64,
    coinbase_tx_suffix: Vec<u8>,
    coinbase_tx_prefix: Vec<u8>,
    extranonce: Vec<u8>,
    #[allow(dead_code)]
    merkle_path: Vec<Vec<u8>>,
    merkle_root: TxMerkleNode,
}

#[derive(Debug)]
pub enum VelideateTargetResult {
    LessThanBitcoinTarget(BlockHash, u64, SubmitSolution<'static>),
    LessThanDownstreamTarget(BlockHash, u64),
    Invalid(BlockHash),
}

impl CompleteJob {
    pub fn get_coinbase(&self) -> B064K<'static> {
        let mut coinbase = Vec::new();
        coinbase.extend(self.coinbase_tx_prefix.clone());
        coinbase.extend(self.extranonce.clone());
        coinbase.extend(self.coinbase_tx_suffix.clone());
        coinbase.try_into().unwrap()
    }
    pub fn validate_target(
        &mut self,
        nonce: u32,
        version: u32,
        ntime: u32,
        extranonce_suffix: Option<&[u8]>,
    ) -> VelideateTargetResult {
        let merkle_root = match extranonce_suffix {
            None => self.merkle_root,
            Some(suffix) => {
                let mid_point = self.extranonce.len() - suffix.len();
                let extranonce = [&self.extranonce[0..mid_point], suffix].concat();
                assert!(self.extranonce.len() == 32);
                let merkle_root: [u8; 32] = merkle_root_from_path(
                    &(self.coinbase_tx_prefix[..]),
                    &(self.coinbase_tx_suffix[..]),
                    &extranonce[..],
                    &(self.merkle_path[..]),
                )
                .unwrap()
                .try_into()
                .unwrap();
                let merkle_root = Hash::from_inner(merkle_root);
                TxMerkleNode::from_hash(merkle_root)
            }
        };
        // TODO  how should version be transoformed from u32 into i32???
        let version = version as i32;
        let header = BlockHeader {
            version,
            prev_blockhash: self.prev_hash,
            merkle_root,
            time: ntime,
            bits: self.nbits,
            nonce,
        };

        let bitcoin_target = header.target();

        let hash_ = header.block_hash();
        let mut hash = hash_.as_hash().into_inner();
        hash.reverse();
        let hash = Uint256::from_be_bytes(hash);
        if hash <= bitcoin_target {
            self.new_shares_sum += 1;
            let solution = SubmitSolution {
                template_id: self.template_id,
                version: version as u32,
                header_timestamp: ntime,
                header_nonce: nonce,
                coinbase_tx: self.get_coinbase(),
            };
            VelideateTargetResult::LessThanBitcoinTarget(hash_, self.new_shares_sum, solution)
        } else if hash <= self.target {
            self.new_shares_sum += 1;
            VelideateTargetResult::LessThanDownstreamTarget(hash_, self.new_shares_sum)
        } else {
            VelideateTargetResult::Invalid(hash_)
        }
    }

    pub fn update_job(
        &self,
        new_ext_job: &NewExtendedMiningJob<'static>,
        nbits: u32,
        prev_hash: BlockHash,
        template_id: u64,
    ) -> Self {
        let merkle_root: [u8; 32] = merkle_root_from_path(
            &(self.coinbase_tx_prefix[..]),
            &(self.coinbase_tx_suffix[..]),
            &(self.extranonce[..]),
            &(new_ext_job.merkle_path.inner_as_ref()[..]),
        )
        .unwrap()
        .try_into()
        .unwrap();
        let merkle_root = Hash::from_inner(merkle_root);
        let merkle_root = TxMerkleNode::from_hash(merkle_root);
        Self {
            target: self.target,
            nbits,
            prev_hash,
            new_shares_sum: 0,
            coinbase_tx_prefix: new_ext_job.coinbase_tx_prefix.to_vec(),
            coinbase_tx_suffix: new_ext_job.coinbase_tx_suffix.to_vec(),
            merkle_path: new_ext_job.merkle_path.to_vec(),
            extranonce: self.extranonce.clone(),
            merkle_root,
            template_id,
        }
    }
}

#[derive(Debug, Clone)]
enum Job {
    Partial(PartialJob),
    Complete(CompleteJob),
}

impl Job {
    pub fn new(target: Uint256, extranonce: Vec<u8>) -> Self {
        Self::Partial(PartialJob { target, extranonce })
    }
    pub fn update_job(
        &mut self,
        new_ext_job: &NewExtendedMiningJob<'static>,
        nbits: u32,
        prev_hash: BlockHash,
        template_id: u64,
    ) {
        match self {
            Job::Partial(p) => {
                *self = Self::Complete(p.to_complete_standard_job(
                    new_ext_job,
                    nbits,
                    prev_hash,
                    template_id,
                ));
            }
            Job::Complete(c) => {
                *self = Self::Complete(c.update_job(new_ext_job, nbits, prev_hash, template_id));
            }
        }
    }

    pub fn make_partial(&mut self) {
        match self {
            Self::Partial(_) => (),
            Self::Complete(c) => {
                *self = Self::Partial(PartialJob {
                    target: c.target,
                    extranonce: c.extranonce.clone(),
                });
            }
        }
    }
}

#[derive(Debug)]
pub struct ExtendedJob {
    #[allow(dead_code)]
    merkle_path: Vec<u8>,
    #[allow(dead_code)]
    nbits: u32,
}

#[derive(Debug)]
pub struct Downstream<L: 'static + Deref + Debug + Send> where L::Target: Logger, L: Sync {
    // Either group or channel id
    id: u32,
    receiver: Receiver<EitherFrame>,
    sender: Sender<EitherFrame>,
    downstream_data: CommonDownstreamData,
    channel_ids: Id,
    extranonces: Arc<Mutex<ExtendedExtranonce>>,
    // channel_id -> Job
    jobs: HashMap<u32, Job>,
    // extended_job_id -> (FutureJob,template_id)
    future_jobs: HashMap<u32, (NewExtendedMiningJob<'static>, u64)>,
    // channel_id -> Prefixes VALID ONLY FOR EXTENDED CHANNELS
    prefixes: HashMap<u32, Vec<u8>>,
    last_prev_hash: Option<BlockHash>,
    last_nbits: Option<u32>,
    // (job,template_id)
    last_valid_extended_job: Option<(NewExtendedMiningJob<'static>, u64)>,
    solution_sender: Sender<SubmitSolution<'static>>,
    logger: Arc<L>,
}

/// Accept downstream connection
pub struct Pool<L: 'static + Deref + Debug + Send> where L::Target: Logger, L: Sync {
    /// Downstreams that are not HOM
    group_downstreams: HashMap<u32, Arc<Mutex<Downstream<L>>>>,
    /// Downstreams that are HOM
    hom_downstreams: HashMap<u32, Arc<Mutex<Downstream<L>>>>,
    hom_ids: Arc<Mutex<Id>>,
    group_ids: Arc<Mutex<Id>>,
    job_creators: Arc<Mutex<JobsCreators>>,
    last_new_prev_hash: Option<SetNewPrevHash<'static>>,
    extranonces: Arc<Mutex<ExtendedExtranonce>>,
    solution_sender: Sender<SubmitSolution<'static>>,
    new_template_processed: bool,
    logger: Arc<L>,
}

impl<L: 'static + Deref + Debug + Send> Downstream<L> where L::Target: Logger, L: Sync {
    pub fn check_target(
        &mut self,
        channel_id: u32,
        nonce: u32,
        version: u32,
        ntime: u32,
        extranonce_suffix: Option<&[u8]>,
    ) -> Result<VelideateTargetResult, ()> {
        let id = channel_id;
        match self.jobs.get_mut(&id) {
            Some(Job::Complete(job)) => {
                let res = job.validate_target(nonce, version, ntime, extranonce_suffix);
                match res {
                    VelideateTargetResult::LessThanBitcoinTarget(_, _, _) => {
                        self.jobs.get_mut(&id).as_mut().unwrap().make_partial();
                    }
                    VelideateTargetResult::LessThanDownstreamTarget(_, _) => (),
                    VelideateTargetResult::Invalid(_) => (),
                };
                Ok(res)
            }
            Some(Job::Partial(_)) => Err(()),
            None => Err(()),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn new(
        mut receiver: Receiver<EitherFrame>,
        mut sender: Sender<EitherFrame>,
        group_ids: Arc<Mutex<Id>>,
        _hom_ids: Arc<Mutex<Id>>,
        job_creators: Arc<Mutex<JobsCreators>>,
        extranonces: Arc<Mutex<ExtendedExtranonce>>,
        last_new_prev_hash: Option<SetNewPrevHash<'static>>,
        solution_sender: Sender<SubmitSolution<'static>>,
        pool: Arc<Mutex<Pool<L>>>,
        logger: Arc<L>,
    ) -> Arc<Mutex<Self>> {
        let setup_connection = Arc::new(Mutex::new(SetupConnectionHandler::new()));
        let downstream_data =
            SetupConnectionHandler::setup(setup_connection, &mut receiver, &mut sender)
                .await
                .unwrap();
        let id = match downstream_data.header_only {
            false => group_ids.safe_lock(|id| id.next()).unwrap(),
            true => {
                //_hom_ids.safe_lock(|id| id.next()).unwrap();
                panic!("Downstream standard channel not supported");
            }
        };

        let extended_jobs = job_creators
            .safe_lock(|j| {
                j.new_group_channel(id, downstream_data.version_rolling)
                    .unwrap()
            })
            .unwrap();

        let mut future_jobs = HashMap::new();
        let mut last_valid_extended_job = None;

        for job in &extended_jobs {
            if job.0.future_job {
                future_jobs.insert(job.0.job_id, (job.0.clone(), job.1));
            } else {
                last_valid_extended_job = Some((job.0.clone(), job.1));
            }
        }

        if last_valid_extended_job.is_none() && last_new_prev_hash.is_some() {
            let template_id = last_new_prev_hash.as_ref().unwrap().template_id;
            let job_id = job_creators
                .safe_lock(|jc| jc.job_id_from_template(template_id, id))
                .unwrap();
            for job in &extended_jobs {
                if job.0.job_id == job_id.unwrap() {
                    last_valid_extended_job = Some((job.0.clone(), template_id));
                    break;
                }
            }
        }

        let self_ = Arc::new(Mutex::new(Downstream {
            id,
            receiver,
            sender,
            downstream_data,
            channel_ids: Id::new(),
            extranonces,
            jobs: HashMap::new(),
            future_jobs,
            last_prev_hash: None,
            last_nbits: None,
            last_valid_extended_job,
            solution_sender,
            prefixes: HashMap::new(),
            logger,
        }));

        for job in extended_jobs {
            Self::send(
                self_.clone(),
                roles_logic_sv2::parsers::Mining::NewExtendedMiningJob(job.0),
            )
            .await
            .unwrap();
        }

        if let Some(new_prev_hash) = last_new_prev_hash {
            let job_id = job_creators
                .safe_lock(|j| j.job_id_from_template(new_prev_hash.template_id, id))
                .unwrap();
            let message = NewPrevHash {
                channel_id: id,
                job_id: job_id.unwrap(),
                prev_hash: new_prev_hash.prev_hash.clone(),
                min_ntime: 0,
                nbits: new_prev_hash.n_bits,
            };
            self_
                .safe_lock(|d| d.on_new_prev_hash_sync(message.clone()))
                .unwrap()
                .unwrap();
            Downstream::send(self_.clone(), Mining::SetNewPrevHash(message))
                .await
                .unwrap();
        };

        let cloned = self_.clone();

        task::spawn(async move {
            loop {
                let receiver = cloned.safe_lock(|d| d.receiver.clone()).unwrap();
                match receiver.recv().await {
                    Ok(received) => {
                        let received: Result<StdFrame, _> = received.try_into();
                        match received {
                            Ok(std_frame) => Downstream::next(cloned.clone(), std_frame).await,
                            _ => todo!(),
                        }
                    }
                    _ => {
                        match downstream_data.header_only {
                            false => {
                                pool.safe_lock(|p| p.group_downstreams.remove(&id).unwrap())
                                    .unwrap();
                            }
                            true => {
                                //_hom_ids.safe_lock(|id| id.next()).unwrap();
                                panic!("Downstream standard channel not supported");
                            }
                        };
                        break;
                    }
                }
                //let incoming: StdFrame = receiver.recv().await.expect("DICOLCALALCLA").try_into().unwrap();
            }
        });
        self_
    }

    pub async fn next(self_mutex: Arc<Mutex<Self>>, mut incoming: StdFrame) {
        let message_type = incoming.get_header().unwrap().msg_type();
        let payload = incoming.payload();
        let next_message_to_send = ParseDownstreamMiningMessages::handle_message_mining(
            self_mutex.clone(),
            message_type,
            payload,
            MiningRoutingLogic::None,
        );
        match next_message_to_send {
            Ok(SendTo::Respond(message)) => {
                Self::send(self_mutex, message).await.unwrap();
            }
            Ok(SendTo::None(_)) => (),
            Ok(_) => panic!(),
            Err(Error::UnexpectedMessage) => todo!(),
            Err(_) => todo!(),
        }
    }

    pub async fn send(
        self_mutex: Arc<Mutex<Self>>,
        message: roles_logic_sv2::parsers::Mining<'static>,
    ) -> Result<(), ()> {
        let sv2_frame: StdFrame = PoolMessages::Mining(message).try_into().unwrap();
        let sender = self_mutex.safe_lock(|self_| self_.sender.clone()).unwrap();
        sender.send(sv2_frame.into()).await.map_err(|_| ())?;
        Ok(())
    }

    pub fn on_new_prev_hash_sync(&mut self, message: NewPrevHash<'static>) -> Result<StdFrame, ()> {
        let prev_hash = message.prev_hash.clone();

        if let Some(future_job) = self.future_jobs.remove(&message.job_id) {
            for job in self.jobs.values_mut() {
                job.update_job(
                    &future_job.0,
                    message.nbits,
                    u256_to_block_hash(prev_hash.clone()),
                    future_job.1,
                );
            }
        }

        self.last_nbits = Some(message.nbits);
        self.last_prev_hash = Some(u256_to_block_hash(prev_hash));
        self.future_jobs = HashMap::new();

        let sv2_frame: StdFrame = PoolMessages::Mining(Mining::SetNewPrevHash(message))
            .try_into()
            .unwrap();
        Ok(sv2_frame)
    }

    pub async fn on_new_prev_hash(
        self_: Arc<Mutex<Self>>,
        message: NewPrevHash<'static>,
    ) -> Result<(), ()> {
        let sv2_frame = self_
            .safe_lock(|s| s.on_new_prev_hash_sync(message))
            .unwrap()?;
        let sender = self_.safe_lock(|self_| self_.sender.clone()).unwrap();

        sender.send(sv2_frame.into()).await.map_err(|_| ())?;

        Ok(())
    }

    pub async fn on_new_extended_job(
        self_: Arc<Mutex<Self>>,
        message: NewExtendedMiningJob<'static>,
        _merkle_path: Vec<Vec<u8>>,
        template_id: u64,
    ) -> Result<(), ()> {
        if !message.future_job {
            self_
                .safe_lock(|s| {
                    for job in s.jobs.values_mut() {
                        job.update_job(
                            &message,
                            s.last_nbits.unwrap(),
                            *s.last_prev_hash.as_ref().unwrap(),
                            template_id,
                        );
                    }
                })
                .unwrap();
        } else {
            self_
                .safe_lock(|s| {
                    s.future_jobs
                        .insert(message.job_id, (message.clone(), template_id))
                })
                .unwrap();
        }

        let sv2_frame: StdFrame = PoolMessages::Mining(Mining::NewExtendedMiningJob(message))
            .try_into()
            .unwrap();

        let sender = self_.safe_lock(|self_| self_.sender.clone()).unwrap();
        sender.send(sv2_frame.into()).await.map_err(|_| ())?;

        Ok(())
    }
}
impl<L: 'static + Deref + Debug + Send> IsDownstream for Downstream<L> where L::Target: Logger, L: Sync {
    fn get_downstream_mining_data(&self) -> CommonDownstreamData {
        self.downstream_data
    }
}

impl<L: 'static + Deref + Debug + Send> IsMiningDownstream for Downstream<L> where L::Target: Logger, L: Sync {}

impl<L: 'static + Deref + Debug + Send> Pool<L> where L::Target: Logger, L: Sync {
    async fn accept_incoming_connection(self_: Arc<Mutex<Pool<L>>>, config: Configuration) {
        let listner = TcpListener::bind(&config.listen_address).await.unwrap();
        while let Ok((stream, _)) = listner.accept().await {
            let solution_sender = self_.safe_lock(|p| p.solution_sender.clone()).unwrap();
            let responder = Responder::from_authority_kp(
                config.authority_public_key.clone().into_inner().as_bytes(),
                config.authority_secret_key.clone().into_inner().as_bytes(),
                std::time::Duration::from_secs(config.cert_validity_sec),
            )
            .unwrap();
            let last_new_prev_hash = self_.safe_lock(|x| x.last_new_prev_hash.clone()).unwrap();
            let (receiver, sender): (Receiver<EitherFrame>, Sender<EitherFrame>) =
                Connection::new(stream, HandshakeRole::Responder(responder)).await;
            let group_ids = self_.safe_lock(|s| s.group_ids.clone()).unwrap();
            let hom_ids = self_.safe_lock(|s| s.hom_ids.clone()).unwrap();
            let job_creators = self_.safe_lock(|s| s.job_creators.clone()).unwrap();
            let extranonces = self_.safe_lock(|s| s.extranonces.clone()).unwrap();
            let logger = self_.safe_lock(|s| s.logger.clone()).unwrap();

            let downstream = Downstream::new(
                receiver,
                sender,
                group_ids,
                hom_ids,
                job_creators,
                extranonces,
                last_new_prev_hash,
                solution_sender,
                self_.clone(),
                logger,
            )
            .await;

            let (is_header_only, channel_id) = downstream
                .safe_lock(|d| (d.downstream_data.header_only, d.id))
                .unwrap();

            self_
                .safe_lock(|p| {
                    if is_header_only {
                        p.hom_downstreams.insert(channel_id, downstream);
                    } else {
                        p.group_downstreams.insert(channel_id, downstream);
                    }
                })
                .unwrap();
        }
    }

    async fn on_new_prev_hash(self_: Arc<Mutex<Self>>, rx: Receiver<SetNewPrevHash<'static>>) {
        while let Ok(new_prev_hash) = rx.recv().await {
            while !self_.safe_lock(|s| s.new_template_processed).unwrap() {
                tokio::time::sleep(std::time::Duration::from_millis(1)).await;
            }
            self_
                .safe_lock(|s| s.new_template_processed = false)
                .unwrap();
            self_
                .safe_lock(|s| {
                    s.job_creators
                        .safe_lock(|jc| jc.on_new_prev_hash(&new_prev_hash))
                        .unwrap()
                })
                .unwrap();
            self_
                .safe_lock(|s| s.last_new_prev_hash = Some(new_prev_hash.clone()))
                .unwrap();
            let hom_downstreams: Vec<Arc<Mutex<Downstream<L>>>> = self_
                .safe_lock(|s| s.hom_downstreams.iter().map(|d| d.1.clone()).collect())
                .unwrap();
            let group_downstreams: Vec<Arc<Mutex<Downstream<L>>>> = self_
                .safe_lock(|s| s.group_downstreams.iter().map(|d| d.1.clone()).collect())
                .unwrap();
            for downstream in [&hom_downstreams[..], &group_downstreams[..]].concat() {
                let channel_id = downstream.safe_lock(|d| d.id).unwrap();
                let job_id = self_
                    .safe_lock(|s| {
                        s.job_creators
                            .safe_lock(|j| {
                                j.job_id_from_template(new_prev_hash.template_id, channel_id)
                            })
                            .unwrap()
                    })
                    .unwrap();
                let message = NewPrevHash {
                    channel_id,
                    job_id: job_id.unwrap(),
                    prev_hash: new_prev_hash.prev_hash.clone(),
                    min_ntime: 0,
                    nbits: new_prev_hash.n_bits,
                };
                Downstream::on_new_prev_hash(downstream.clone(), message)
                    .await
                    .unwrap();
            }
        }
    }

    async fn on_new_template(self_: Arc<Mutex<Self>>, rx: Receiver<NewTemplate<'_>>) {
        while let Ok(mut new_template) = rx.recv().await {
            let job_creators = self_.safe_lock(|s| s.job_creators.clone()).unwrap();
            let mut new_jobs = job_creators
                .safe_lock(|j| j.on_new_template(&mut new_template).unwrap())
                .unwrap();
            let group_downstreams: Vec<Arc<Mutex<Downstream<L>>>> = self_
                .safe_lock(|s| s.group_downstreams.iter().map(|d| d.1.clone()).collect())
                .unwrap();
            // TODO add standard channel downstream
            for downstream in group_downstreams {
                let channel_id = downstream.safe_lock(|x| x.id).unwrap();
                let extended_job = new_jobs.remove(&channel_id).unwrap();
                Downstream::on_new_extended_job(
                    downstream,
                    extended_job,
                    new_template.merkle_path.to_vec(),
                    new_template.template_id,
                )
                .await
                .unwrap();
            }
            self_
                .safe_lock(|s| s.new_template_processed = true)
                .unwrap();
        }
    }

    pub async fn start(
        logger: Arc<L>,
        config: Configuration,
        new_template_rx: Receiver<NewTemplate<'static>>,
        new_prev_hash_rx: Receiver<SetNewPrevHash<'static>>,
        solution_sender: Sender<SubmitSolution<'static>>,
    ) {
        log_info!(logger, "Starting mining pool");
        //let group_id_generator = Arc::new(Mutex::new(Id::new()));
        let range_0 = std::ops::Range { start: 0, end: 0 };
        let range_1 = std::ops::Range { start: 0, end: 16 };
        let range_2 = std::ops::Range { start: 16, end: 32 };
        let pool = Arc::new(Mutex::new(Pool {
            group_downstreams: HashMap::new(),
            hom_downstreams: HashMap::new(),
            hom_ids: Arc::new(Mutex::new(Id::new())),
            group_ids: Arc::new(Mutex::new(Id::new())),
            job_creators: Arc::new(Mutex::new(
                JobsCreators::new(crate::BLOCK_REWARD, crate::new_pub_key()).unwrap(),
            )),
            last_new_prev_hash: None,
            extranonces: Arc::new(Mutex::new(ExtendedExtranonce::new(
                range_0, range_1, range_2,
            ))),
            solution_sender,
            new_template_processed: false,
            logger
        }));

        let cloned = pool.clone();
        let cloned2 = pool.clone();
        let cloned3 = pool.clone();

        task::spawn(Self::accept_incoming_connection(cloned, config));

        task::spawn(async {
            Self::on_new_prev_hash(cloned2, new_prev_hash_rx).await;
        });

        let _ = task::spawn(async move {
            Self::on_new_template(cloned3, new_template_rx).await;
        })
        .await;
    }
}
