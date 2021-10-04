use async_trait::async_trait;

use crate::{
    crypto::{primitives::sign::PublicKey, KeyChain},
    link::rendezvous::{
        errors::{
            connector::{
                AuthenticateFailed, ConnectionFailed, SecureFailed,
                UnexpectedRemote,
            },
            ConnectorError,
        },
        Client, ConnectorSettings,
    },
    net::{traits::TcpConnect, Connector as NetConnector, SecureConnection},
};

use snafu::ResultExt;

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

pub struct Connector {
    client: Client,
    keychain: KeyChain,
    database: Arc<Mutex<Database>>,
}

struct Database {
    cache: HashMap<PublicKey, SocketAddr>,
}

impl Connector {
    pub fn new<S>(
        server: S,
        keychain: KeyChain,
        settings: ConnectorSettings,
    ) -> Self
    where
        S: 'static + TcpConnect,
    {
        let client = Client::new(server, settings.client_settings);

        let database = Arc::new(Mutex::new(Database {
            cache: HashMap::new(),
        }));

        Connector {
            client,
            keychain,
            database,
        }
    }

    async fn attempt(
        &self,
        root: PublicKey,
    ) -> Result<SecureConnection, ConnectorError> {
        let address = self
            .get_address(root)
            .ok_or(ConnectorError::AddressUnknown)?;

        let mut connection = address
            .connect()
            .await
            .context(ConnectionFailed)?
            .secure()
            .await
            .context(SecureFailed)?;

        let keycard = connection
            .authenticate(&self.keychain)
            .await
            .context(AuthenticateFailed)?;

        if keycard.root() == root {
            Ok(connection)
        } else {
            UnexpectedRemote {
                remote: keycard.root(),
            }
            .fail()
        }
    }

    async fn refresh(&self, root: PublicKey) -> bool {
        let stale = self.get_address(root);
        let fresh = self.client.get_address(root).await.ok().or(stale.clone());

        if fresh != stale {
            self.cache_address(root, fresh.unwrap()); // `fresh` can be `None` only if `stale` is `None` too
            true
        } else {
            false
        }
    }

    fn get_address(&self, root: PublicKey) -> Option<SocketAddr> {
        self.database
            .lock()
            .unwrap()
            .cache
            .get(&root)
            .map(Clone::clone)
    }

    fn cache_address(&self, root: PublicKey, address: SocketAddr) {
        self.database.lock().unwrap().cache.insert(root, address);
    }
}

#[async_trait]
impl NetConnector for Connector {
    type Error = ConnectorError;

    async fn connect(
        &self,
        root: PublicKey,
    ) -> Result<SecureConnection, ConnectorError> {
        loop {
            let result = self.attempt(root).await;

            if result.is_ok() || !self.refresh(root).await {
                return result;
            }
        }
    }
}