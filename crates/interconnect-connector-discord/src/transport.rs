//! Discord gateway transport.
//!
//! Presents a Discord channel as an Interconnect `Transport`. Gateway events
//! become `ServerWire<DiscordSnapshot>` bytes; `ClientWire<DiscordIntent>`
//! bytes become HTTP API calls.

use crate::types::{DiscordError, DiscordIntent, DiscordMessage, DiscordSnapshot};
use interconnect_core::{ClientWire, ServerWire, Transport};
use std::collections::VecDeque;
use std::sync::Arc;
use twilight_gateway::{EventTypeFlags, Intents, Shard, ShardId};
use twilight_http::Client;
use twilight_model::gateway::event::Event;
use twilight_model::id::{Id, marker::ChannelMarker};

pub const MAX_MESSAGES: usize = 50;

pub struct DiscordTransport {
    pub(crate) shard: Shard,
    pub(crate) http: Arc<Client>,
    pub(crate) channel_id: Id<ChannelMarker>,
    pub(crate) channel_name: String,
    pub(crate) messages: VecDeque<DiscordMessage>,
    pub(crate) seq: u64,
}

impl DiscordTransport {
    pub(crate) fn current_snapshot(&self) -> DiscordSnapshot {
        DiscordSnapshot {
            channel_id: self.channel_id.to_string(),
            channel_name: self.channel_name.clone(),
            messages: self.messages.iter().cloned().collect(),
        }
    }

    /// Fetch recent messages via HTTP and populate the internal buffer.
    pub(crate) async fn fetch_initial_snapshot(&mut self) -> Result<DiscordSnapshot, DiscordError> {
        let messages = self
            .http
            .channel_messages(self.channel_id)
            .limit(MAX_MESSAGES as u16)
            .await?
            .model()
            .await?;

        // API returns newest-first; reverse for oldest-first display.
        for msg in messages.into_iter().rev() {
            // Derive timestamp from Discord snowflake: ms = (id >> 22) + Discord epoch
            let timestamp = (msg.id.get() >> 22).saturating_add(1_420_070_400_000) / 1_000;
            self.messages.push_back(DiscordMessage {
                id: msg.id.to_string(),
                author_id: msg.author.id.to_string(),
                author_name: msg.author.name.clone(),
                content: msg.content.clone(),
                timestamp,
            });
        }

        Ok(self.current_snapshot())
    }

    fn push_message(&mut self, msg: DiscordMessage) {
        self.messages.push_back(msg);
        if self.messages.len() > MAX_MESSAGES {
            self.messages.pop_front();
        }
    }
}

impl Transport for DiscordTransport {
    type Error = DiscordError;

    async fn send(&mut self, data: &[u8]) -> Result<(), Self::Error> {
        let wire: ClientWire<DiscordIntent> = serde_json::from_slice(data)?;
        match wire {
            ClientWire::Intent(DiscordIntent::SendMessage { content }) => {
                self.http
                    .create_message(self.channel_id)
                    .content(&content)
                    .await?;
            }
            ClientWire::Intent(DiscordIntent::React { message_id, emoji }) => {
                if let Ok(id) = message_id.parse::<u64>() {
                    let msg_id = Id::new(id);
                    self.http
                        .create_reaction(self.channel_id, msg_id, &twilight_http::request::channel::reaction::RequestReactionType::Unicode { name: &emoji })
                        .await?;
                }
            }
            // Auth, Ping, TransferRequest — not applicable for platform connectors.
            _ => {}
        }
        Ok(())
    }

    async fn recv(&mut self) -> Result<Option<Vec<u8>>, Self::Error> {
        use twilight_gateway::StreamExt as _;
        loop {
            let event = match self.shard.next_event(EventTypeFlags::MESSAGE_CREATE).await {
                Some(Ok(e)) => e,
                Some(Err(e)) => return Err(DiscordError::Gateway(e)),
                None => return Ok(None),
            };

            match event {
                Event::MessageCreate(msg) if msg.channel_id == self.channel_id => {
                    let timestamp =
                        (msg.id.get() >> 22).saturating_add(1_420_070_400_000) / 1_000;
                    self.push_message(DiscordMessage {
                        id: msg.id.to_string(),
                        author_id: msg.author.id.to_string(),
                        author_name: msg.author.name.clone(),
                        content: msg.content.clone(),
                        timestamp,
                    });

                    let snapshot = self.current_snapshot();
                    let wire = ServerWire::<DiscordSnapshot>::Snapshot {
                        seq: self.seq,
                        data: snapshot,
                    };
                    self.seq += 1;
                    return Ok(Some(serde_json::to_vec(&wire)?));
                }
                _ => continue,
            }
        }
    }
}

/// Create a shard configured for message events in a single guild.
pub fn make_shard(token: String) -> Shard {
    Shard::new(
        ShardId::ONE,
        token,
        Intents::GUILD_MESSAGES | Intents::MESSAGE_CONTENT,
    )
}
