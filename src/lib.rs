use async_channel::{unbounded, Receiver, Sender, TryRecvError};
use bevy::{
    prelude::*,
    tasks::{IoTaskPool, TaskPool},
};
use web3::transports::eip_1193;

pub use web3::types::{H160, H520};

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
        app.add_systems(Startup, init_wallet);
    }
}

#[derive(Resource)]
pub struct WalletChannel {
    account_tx: Sender<(H160, String)>,
    account_rx: Receiver<(H160, String)>,
    signature_tx: Sender<H520>,
    signature_rx: Receiver<H520>,
    transaction_tx: Sender<H520>,
    transaction_rx: Receiver<H520>,
}

fn init_wallet(mut commands: Commands) {
    let (account_tx, account_rx) = unbounded();
    let (signature_tx, signature_rx) = unbounded();
    let (transaction_tx, transaction_rx) = unbounded();

    commands.insert_resource(WalletChannel {
        account_tx,
        account_rx,
        signature_tx,
        signature_rx,
        transaction_tx,
        transaction_rx,
    });
}

impl WalletChannel {
    pub fn connect(&self) {
        let tx = self.account_tx.clone();
        IoTaskPool::get_or_init(TaskPool::new)
            .spawn(async move {
                let provider = eip_1193::Provider::default().unwrap().unwrap();
                let transport = eip_1193::Eip1193::new(provider);
                let web3 = web3::Web3::new(transport);

                let addrs = web3.eth().request_accounts().await.unwrap();

                if !addrs.is_empty() {
                    info!("addrs: {:?}", addrs);
                    let _ = tx.send((addrs[0], "TODO_network".to_owned())).await;
                }
            })
            .detach();
    }

    pub fn sign(&self, account: H160, msg: String) {
        let tx = self.signature_tx.clone();
        IoTaskPool::get_or_init(TaskPool::new)
            .spawn(async move {
                let provider = eip_1193::Provider::default().unwrap().unwrap();
                let transport = eip_1193::Eip1193::new(provider);
                let web3 = web3::Web3::new(transport);

                let msg = web3::types::Bytes(msg.as_bytes().to_vec());
                let signature = web3.personal().sign(msg, account, "").await.unwrap();
                let _ = tx.send(signature).await;
            })
            .detach();
    }

    pub fn send(&self) {
        let _tx = self.transaction_tx.clone();
        IoTaskPool::get_or_init(TaskPool::new)
            .spawn(async move {
                let provider = eip_1193::Provider::default().unwrap().unwrap();
                let transport = eip_1193::Eip1193::new(provider);
                let _web3 = web3::Web3::new(transport);

                // TODO
            })
            .detach();
    }

    pub fn recv_account(&self) -> Result<(H160, String), RecvError> {
        Ok(self.account_rx.try_recv()?)
    }

    pub fn recv_signature(&self) -> Result<H520, RecvError> {
        Ok(self.signature_rx.try_recv()?)
    }

    pub fn recv_transaction(&self) -> Result<H520, RecvError> {
        Ok(self.transaction_rx.try_recv()?)
    }
}
