use async_channel::{unbounded, Receiver, Sender, TryRecvError};
use bevy::{
    prelude::*,
    tasks::{IoTaskPool, TaskPool},
};
use chamomile_types::PeerId;
use web3::{
    ethabi::Contract as EthContract,
    transports::eip_1193,
    types::{CallRequest, TransactionRequest},
};

pub use web3::{
    ethabi::Token,
    types::{H160, H256, H520, U256},
};

pub enum RecvError {
    Empty,
    Closed,
}

impl From<TryRecvError> for RecvError {
    fn from(e: TryRecvError) -> RecvError {
        match e {
            TryRecvError::Empty => RecvError::Empty,
            TryRecvError::Closed => RecvError::Closed,
        }
    }
}

pub struct WalletPlugin;

impl Plugin for WalletPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, init_eth_wallet);
    }
}

#[derive(Resource)]
pub struct EthWallet {
    accounts: Vec<H160>,
    chain_id: u64,
    account_tx: Sender<(Vec<H160>, u64)>,
    account_rx: Receiver<(Vec<H160>, u64)>,
    signature_tx: Sender<H520>,
    signature_rx: Receiver<H520>,
    transaction_tx: Sender<H256>,
    transaction_rx: Receiver<H256>,
    call_tx: Sender<Vec<u8>>,
    call_rx: Receiver<Vec<u8>>,
}

fn init_eth_wallet(mut commands: Commands) {
    let (account_tx, account_rx) = unbounded();
    let (signature_tx, signature_rx) = unbounded();
    let (transaction_tx, transaction_rx) = unbounded();
    let (call_tx, call_rx) = unbounded();

    commands.insert_resource(EthWallet {
        accounts: vec![],
        chain_id: 0,
        account_tx,
        account_rx,
        signature_tx,
        signature_rx,
        transaction_tx,
        transaction_rx,
        call_tx,
        call_rx,
    });
}

impl EthWallet {
    pub fn connect(&self) {
        let tx = self.account_tx.clone();
        IoTaskPool::get_or_init(TaskPool::new)
            .spawn(async move {
                let provider = eip_1193::Provider::default().unwrap().unwrap();
                let transport = eip_1193::Eip1193::new(provider);
                let web3 = web3::Web3::new(transport);

                let addrs = web3.eth().request_accounts().await.unwrap();
                let chain = web3.eth().chain_id().await.unwrap();

                if !addrs.is_empty() {
                    let _ = tx.send((addrs, chain.as_u64())).await;
                }
            })
            .detach();
    }

    pub fn sign(&self, account: &str, msg: String) {
        let account = account.parse().unwrap();

        let tx = self.signature_tx.clone();
        IoTaskPool::get_or_init(TaskPool::new)
            .spawn(async move {
                let provider = eip_1193::Provider::default().unwrap().unwrap();
                let transport = eip_1193::Eip1193::new(provider);
                let web3 = web3::Web3::new(transport);

                let msg = web3::types::Bytes(msg.as_bytes().to_vec());
                let signature = web3.eth().sign(account, msg).await.unwrap();
                let _ = tx.send(signature).await;
            })
            .detach();
    }

    pub fn send(&self, from: &str, to: H160, data: Vec<u8>) {
        let from = from.parse().unwrap();

        let tx = self.transaction_tx.clone();
        IoTaskPool::get_or_init(TaskPool::new)
            .spawn(async move {
                let provider = eip_1193::Provider::default().unwrap().unwrap();
                let transport = eip_1193::Eip1193::new(provider);
                let web3 = web3::Web3::new(transport);

                let mut txr = TransactionRequest::default();
                txr.from = from;
                txr.to = Some(to);
                txr.data = Some(data.into());

                let hash = web3.eth().send_transaction(txr).await.unwrap();
                let _ = tx.send(hash).await;
            })
            .detach();
    }

    pub fn call(&self, to: H160, data: Vec<u8>) {
        let tx = self.call_tx.clone();
        IoTaskPool::get_or_init(TaskPool::new)
            .spawn(async move {
                let provider = eip_1193::Provider::default().unwrap().unwrap();
                let transport = eip_1193::Eip1193::new(provider);
                let web3 = web3::Web3::new(transport);

                let mut call = CallRequest::default();
                call.to = Some(to);
                call.data = Some(data.into());

                let bytes = web3.eth().call(call, None).await.unwrap();
                let _ = tx.send(bytes.0).await;
            })
            .detach();
    }

    pub fn recv_account(&mut self) -> Result<(String, u64), RecvError> {
        let (addrs, chain) = self.account_rx.try_recv()?;
        self.accounts = addrs;
        self.chain_id = chain;

        let addr = PeerId(self.accounts[0].to_fixed_bytes());
        Ok((addr.to_hex(), chain))
    }

    pub fn recv_signature(&self) -> Result<H520, RecvError> {
        Ok(self.signature_rx.try_recv()?)
    }

    pub fn recv_transaction(&self) -> Result<H256, RecvError> {
        Ok(self.transaction_rx.try_recv()?)
    }

    pub fn recv_call(&self) -> Result<Vec<u8>, RecvError> {
        Ok(self.call_rx.try_recv()?)
    }
}

pub struct Contract {
    pub address: H160,
    abi: EthContract,
}

impl Contract {
    pub fn load(address: &str, json: &[u8]) -> Self {
        let address = address.parse().unwrap();
        let abi = EthContract::load(json).unwrap();
        Contract { address, abi }
    }

    pub fn encode(&self, method: &str, tokens: &[Token]) -> Vec<u8> {
        self.abi
            .function(method)
            .unwrap()
            .encode_input(tokens)
            .unwrap()
    }

    pub fn decode(&self, method: &str, bytes: &[u8]) -> Vec<Token> {
        self.abi
            .function(method)
            .unwrap()
            .decode_output(bytes)
            .unwrap()
            .into()
    }
}
