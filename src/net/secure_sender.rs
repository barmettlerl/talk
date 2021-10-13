use crate::{
    crypto::primitives::channel::Sender as ChannelSender,
    net::{SecureConnectionError, UnitSender},
};

use doomstack::{here, Doom, ResultExt, Top};

use serde::Serialize;

pub struct SecureSender {
    unit_sender: UnitSender,
    channel_sender: ChannelSender,
}

impl SecureSender {
    pub(in crate::net) fn new(
        unit_sender: UnitSender,
        channel_sender: ChannelSender,
    ) -> Self {
        Self {
            unit_sender,
            channel_sender,
        }
    }

    pub async fn send<M>(
        &mut self,
        message: &M,
    ) -> Result<(), Top<SecureConnectionError>>
    where
        M: Serialize,
    {
        self.channel_sender
            .encrypt_into(message, self.unit_sender.as_vec())
            .pot(SecureConnectionError::EncryptFailed, here!())?;

        self.unit_sender
            .flush()
            .await
            .map_err(SecureConnectionError::write_failed)
            .map_err(Doom::into_top)
            .spot(here!())
    }

    pub async fn send_plain<M>(
        &mut self,
        message: &M,
    ) -> Result<(), Top<SecureConnectionError>>
    where
        M: Serialize,
    {
        self.channel_sender
            .authenticate_into(message, self.unit_sender.as_vec())
            .pot(SecureConnectionError::MacComputeFailed, here!())?;

        self.unit_sender
            .flush()
            .await
            .map_err(SecureConnectionError::write_failed)
            .map_err(Doom::into_top)
            .spot(here!())
    }
}
