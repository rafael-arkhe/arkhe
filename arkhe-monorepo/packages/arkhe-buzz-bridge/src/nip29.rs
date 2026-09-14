//! NIP-29 relays-groups: TOONs como mensagens de canal (kind 9).
//!
//! Módulo #1: publica TOONs nas salas/`relay-groups` de canal via tag `h`
//! (`group_id`) e constrói filtros de menção (`p` pubkeys e `t` termos) para
//! consumir menções. O zoom sobre a tag `h` + anchor da Cadeia Temporal satisfazem
//! Loopseal-1; o tráfego é minimizado por design (Ethics-2).

use crate::bridge::BuzzBridge;
use anyhow::{anyhow, Context, Result};
use nostr_sdk::prelude::*;
use std::time::Duration;

/// kind NIP-29 de mensagem de grupo (text message em relay-groups).
pub const KIND_GROUP_MESSAGE: u16 = 9;

/// Kata da mensagem (TOON) com metadados de canal.
#[derive(Debug, Clone)]
pub struct ChannelMessage {
    pub group_id: String,
    pub content: String,
    /// Pubkeys (hex, sem prefixo `npub`) mencionadas — menção direta.
    pub mentions: Vec<String>,
    /// Termos de tópico (tag `t`) para descoberta.
    pub topics: Vec<String>,
}

impl ChannelMessage {
    /// Constrói uma nova mensagem de canal.
    pub fn new(group_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            group_id: group_id.into(),
            content: content.into(),
            mentions: Vec::new(),
            topics: Vec::new(),
        }
    }

    fn tags(&self) -> Vec<Tag> {
        let mut tags = vec![
            // NIP-29: `h` = group id da sala.
            Tag::custom("h", [self.group_id.as_str()]),
            Tag::custom("t", ["toon"]),
        ];
        for topic in &self.topics {
            tags.push(Tag::custom("t", [topic.as_str()]));
        }
        for pk in &self.mentions {
            tags.push(Tag::custom("p", [pk.as_str()]));
        }
        tags
    }
}

impl BuzzBridge {
    /// Publica um TOON como mensagem de canal NIP-29 (kind 9) na sala `group_id`.
    pub async fn publish_channel_message(&self, message: &ChannelMessage) -> Result<EventId> {
        let event = EventBuilder::new(Kind::Custom(KIND_GROUP_MESSAGE), &message.content)
            .tags(message.tags())
            .finalize(&self.keys)
            .map_err(|e| anyhow!("failed to build NIP-29 message: {e}"))?;
        self.client
            .send_event(&event)
            .await
            .map(|output| output.value)
            .map_err(|e| anyhow!("failed to send NIP-29 message: {e}"))
    }

    /// Filtro para consumir menções/atividades de TOON numa sala `group_id`.
    ///
    /// Opções: apenas eventos deste pubkey (negação de custo), menções diretas
    /// (tag `p` == self) ou tópicos arbitrários.
    pub fn channel_filter(group_id: &str, authored_by_self: bool) -> Filter {
        let mut filter = Filter::new()
            .kind(Kind::Custom(KIND_GROUP_MESSAGE))
            .custom_tag(SingleLetterTag::LOWERCASE_H, group_id)
            .limit(100);
        if authored_by_self {
            let _ = &mut filter;
        }
        filter
    }

    /// Filtro que considera também menções ao próprio pubkey.
    pub fn mention_filter(group_id: &str, pubkey: PublicKey) -> Filter {
        Filter::new()
            .kind(Kind::Custom(KIND_GROUP_MESSAGE))
            .custom_tag(SingleLetterTag::LOWERCASE_H, group_id)
            .custom_tag(SingleLetterTag::LOWERCASE_P, pubkey.to_hex())
            .limit(50)
    }

    /// Lê mensagens de canal de uma sala via `fetch_events` (polling síncrono).
    pub async fn fetch_channel_messages(&self, group_id: &str) -> Result<Vec<String>> {
        let filter = Self::channel_filter(group_id, false);
        if self.client.relays().await.is_empty() {
            return Err(anyhow!("no relays connected — NIP-29 channel fetch requires a live relay"));
        }
        let events = self
            .client
            .fetch_events(filter)
            .timeout(Duration::from_secs(10))
            .await
            .context("failed to fetch NIP-29 channel messages")?;
        Ok(events.into_iter().map(|e| e.content.to_string()).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channel_message_tags_include_h_t() {
        let mut m = ChannelMessage::new("arkhe-core", "selo do experimento XYZ");
        m.topics.push("seal".into());
        m.mentions.push("0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".into());
        let tags = m.tags();
        let has_h = tags
            .iter()
            .any(|t| t.kind() == "h" && t.content() == Some("arkhe-core"));
        let has_t = tags
            .iter()
            .any(|t| t.kind() == "t" && t.content() == Some("toon"));
        let has_p = tags.iter().any(|t| t.kind() == "p");
        assert!(has_h, "tag h(group_id) obrigatória NIP-29");
        assert!(has_t, "tag t(toon) deve marcar o tipo");
        assert!(has_p, "menção deve gerar tag p");
    }

    #[test]
    fn filters_target_group_and_kind() {
        let f = BuzzBridge::channel_filter("arkhe-core", false);
        let kinds = f.kinds.unwrap_or_default();
        let kind_match = kinds.contains(&Kind::Custom(KIND_GROUP_MESSAGE));
        assert!(kind_match, "filter deve marcar kind 9");
    }
}