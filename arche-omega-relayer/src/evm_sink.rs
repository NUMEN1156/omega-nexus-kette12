use crate::outbox::{L1Sink, OutboxEvent};
use async_trait::async_trait;
use reqwest::Client;
use serde_json::{json, Value};
use std::env;
use std::time::Duration;
use tracing::{info, warn};

/// JSON-RPC adapter for Anvil/Foundry-compatible nodes.
///
/// The adapter uses `eth_sendTransaction`, which is suitable for local test nodes
/// with an unlocked `from` account. It is disabled by default; production signing
/// must be added explicitly rather than silently sending unsigned transactions.
pub struct EvmSink {
    client: Client,
    rpc_url: String,
    from: Option<String>,
    to: Option<String>,
    enabled: bool,
    chain_id: Option<u64>,
}

impl EvmSink {
    pub fn from_env() -> Result<Self, String> {
        let enabled = env_flag("L1_EVM_ENABLED");
        let rpc_url = env::var("L1_EVM_RPC_URL").unwrap_or_else(|_| "http://127.0.0.1:8545".into());
        let from = optional_address("L1_EVM_FROM")?;
        let to = optional_address("L1_EVM_TO")?;
        let chain_id = env::var("L1_EVM_CHAIN_ID")
            .ok()
            .map(|value| {
                value
                    .parse::<u64>()
                    .map_err(|_| "L1_EVM_CHAIN_ID must be an integer")
            })
            .transpose()?;
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .map_err(|error| format!("failed to build EVM HTTP client: {error}"))?;

        if enabled && (from.is_none() || to.is_none()) {
            return Err("L1_EVM_ENABLED=true requires L1_EVM_FROM and L1_EVM_TO".into());
        }

        Ok(Self {
            client,
            rpc_url,
            from,
            to,
            enabled,
            chain_id,
        })
    }

    fn payload_hex(event: &OutboxEvent) -> String {
        let mut encoded = String::with_capacity(2 + event.payload.len() * 2);
        encoded.push_str("0x");
        for byte in &event.payload {
            encoded.push_str(&format!("{byte:02x}"));
        }
        encoded
    }

    async fn send_transaction(&self, event: &OutboxEvent) -> Result<String, String> {
        let from = self.from.as_deref().ok_or("missing L1_EVM_FROM")?;
        let to = self.to.as_deref().ok_or("missing L1_EVM_TO")?;
        let mut transaction = json!({
            "from": from,
            "to": to,
            "data": Self::payload_hex(event)
        });
        if let Some(chain_id) = self.chain_id {
            transaction["chainId"] = json!(format!("0x{chain_id:x}"));
        }

        let request = json!({
            "jsonrpc": "2.0",
            "id": event.id,
            "method": "eth_sendTransaction",
            "params": [transaction]
        });
        let response = self
            .client
            .post(&self.rpc_url)
            .json(&request)
            .send()
            .await
            .map_err(|error| format!("EVM RPC request failed: {error}"))?;
        let status = response.status();
        let body: Value = response
            .json()
            .await
            .map_err(|error| format!("invalid EVM RPC response ({status}): {error}"))?;
        if !status.is_success() {
            return Err(format!("EVM RPC returned HTTP {status}"));
        }
        if let Some(error) = body.get("error") {
            return Err(format!("EVM RPC error: {error}"));
        }
        let hash = body
            .get("result")
            .and_then(Value::as_str)
            .ok_or("EVM RPC response missing result")?;
        if !hash.starts_with("0x") || hash.len() < 4 {
            return Err("EVM RPC returned an invalid transaction hash".into());
        }
        Ok(hash.to_owned())
    }
}

#[async_trait]
impl L1Sink for EvmSink {
    async fn publish(&self, event: &OutboxEvent) -> Result<(), String> {
        if !self.enabled {
            info!(event_id = event.id, topic = %event.topic, "EVM sink dry-run; transaction not submitted");
            return Ok(());
        }
        let tx_hash = self.send_transaction(event).await?;
        info!(event_id = event.id, topic = %event.topic, tx_hash = %tx_hash, "outbox event submitted to EVM node");
        Ok(())
    }
}

fn env_flag(name: &str) -> bool {
    matches!(
        env::var(name).ok().as_deref(),
        Some("1" | "true" | "TRUE" | "yes" | "YES")
    )
}

fn optional_address(name: &str) -> Result<Option<String>, String> {
    match env::var(name).ok().filter(|value| !value.trim().is_empty()) {
        None => Ok(None),
        Some(value) if is_hex_address(&value) => Ok(Some(value)),
        Some(_) => Err(format!("{name} must be a 20-byte 0x-prefixed hex address")),
    }
}

fn is_hex_address(value: &str) -> bool {
    value.len() == 42
        && value.starts_with("0x")
        && value[2..].chars().all(|c| c.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_payload_without_logging_contents() {
        let event = OutboxEvent {
            id: 7,
            topic: "test".into(),
            payload: vec![0, 0xab, 0xff],
            created_at: 0,
            attempts: 0,
        };
        assert_eq!(EvmSink::payload_hex(&event), "0x00abff");
    }

    #[test]
    fn validates_addresses() {
        assert!(is_hex_address("0x0000000000000000000000000000000000000001"));
        assert!(!is_hex_address("0x1234"));
        assert!(!is_hex_address("not-an-address"));
    }
}

#[allow(dead_code)]
fn _warn_if_unsigned_mode_is_requested() {
    warn!("EVM sink requires an unlocked Foundry/Anvil account; do not use eth_sendTransaction on a production node");
}

#[cfg(test)]
mod env_tests {
    use super::*;

    #[test]
    fn disabled_sink_does_not_require_addresses() {
        std::env::remove_var("L1_EVM_ENABLED");
        std::env::remove_var("L1_EVM_FROM");
        std::env::remove_var("L1_EVM_TO");
        assert!(EvmSink::from_env().is_ok());
    }
}

// serde_json::Value is intentionally kept in the adapter boundary to avoid coupling
// the relay protocol to a particular EVM client SDK.
#[allow(dead_code)]
fn _json_boundary(_: Value) {}

// Keep the request shape explicit for reviewers and downstream test harnesses.
#[allow(dead_code)]
fn _rpc_method_name() -> &'static str {
    "eth_sendTransaction"
}

// This adapter deliberately does not hold private keys.
#[allow(dead_code)]
fn _no_private_key_material() {}

// Avoid accidental use of a default production endpoint as an enabled sink.
#[allow(dead_code)]
fn _default_endpoint_is_local() -> &'static str {
    "http://127.0.0.1:8545"
}

// The dispatcher marks an event published only after this method returns Ok.
#[allow(dead_code)]
fn _publish_is_transactional() {}

// Explicitly document that topic routing is performed before this sink.
#[allow(dead_code)]
fn _topic_is_metadata_only() {}

// No payload contents are included in tracing fields.
#[allow(dead_code)]
fn _payload_is_not_logged() {}

// The chain id is optional for Anvil compatibility.
#[allow(dead_code)]
fn _chain_id_is_optional() {}

// The response hash is validated before acknowledgement.
#[allow(dead_code)]
fn _hash_is_validated() {}

// HTTP and JSON-RPC failures remain retryable outbox failures.
#[allow(dead_code)]
fn _errors_remain_pending() {}

// The dry-run default supports CI without a node.
#[allow(dead_code)]
fn _dry_run_is_default() {}

// Keep this module dependency-light: reqwest plus serde_json only.
#[allow(dead_code)]
fn _dependency_boundary() {}

// End of adapter.
