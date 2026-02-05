//! Terra Event Parsing
//!
//! Provides utilities for parsing CosmWasm transaction events from Terra LCD responses.

use crate::types::ChainId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Parsed wasm event from a Terra transaction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmEvent {
    /// Contract address that emitted the event
    pub contract_address: String,
    /// Event type (action)
    pub action: String,
    /// Event attributes as key-value pairs
    pub attributes: HashMap<String, String>,
}

impl WasmEvent {
    /// Parse wasm events from transaction events
    pub fn from_tx_events(events: &[TxEvent]) -> Vec<Self> {
        let mut result = Vec::new();

        for event in events {
            if event.event_type == "wasm" {
                let mut attrs: HashMap<String, String> = HashMap::new();
                let mut contract_address = String::new();
                let mut action = String::new();

                for attr in &event.attributes {
                    if attr.key == "_contract_address" {
                        contract_address = attr.value.clone();
                    } else if attr.key == "action" {
                        action = attr.value.clone();
                    } else {
                        attrs.insert(attr.key.clone(), attr.value.clone());
                    }
                }

                if !contract_address.is_empty() {
                    result.push(WasmEvent {
                        contract_address,
                        action,
                        attributes: attrs,
                    });
                }
            }
        }

        result
    }

    /// Get an attribute value by key
    pub fn get(&self, key: &str) -> Option<&String> {
        self.attributes.get(key)
    }

    /// Check if this is a specific action
    pub fn is_action(&self, action: &str) -> bool {
        self.action == action
    }
}

/// Raw transaction event from LCD
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TxEvent {
    #[serde(rename = "type")]
    pub event_type: String,
    pub attributes: Vec<TxEventAttribute>,
}

/// Raw event attribute
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TxEventAttribute {
    pub key: String,
    pub value: String,
}

/// V2 Deposit event from Terra bridge
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerraDepositEvent {
    /// Destination chain ID (4 bytes)
    pub dest_chain: ChainId,
    /// Destination account (32 bytes universal address, base64 encoded)
    pub dest_account: String,
    /// Token (denom or CW20 address)
    pub token: String,
    /// Amount deposited
    pub amount: u128,
    /// Deposit nonce
    pub nonce: u64,
    /// Fee charged
    pub fee: u128,
    /// Transaction hash
    pub tx_hash: String,
    /// Block height
    pub height: u64,
}

impl TerraDepositEvent {
    /// Parse from WasmEvent
    pub fn from_wasm_event(event: &WasmEvent, tx_hash: String, height: u64) -> Option<Self> {
        if !event.is_action("deposit") {
            return None;
        }

        let dest_chain_b64 = event.get("dest_chain")?;
        let dest_chain_bytes =
            base64::Engine::decode(&base64::engine::general_purpose::STANDARD, dest_chain_b64)
                .ok()?;
        if dest_chain_bytes.len() != 4 {
            return None;
        }
        let mut chain_bytes = [0u8; 4];
        chain_bytes.copy_from_slice(&dest_chain_bytes);

        Some(TerraDepositEvent {
            dest_chain: ChainId::from_bytes(chain_bytes),
            dest_account: event.get("dest_account")?.clone(),
            token: event.get("token")?.clone(),
            amount: event.get("amount")?.parse().ok()?,
            nonce: event.get("nonce")?.parse().ok()?,
            fee: event.get("fee")?.parse().unwrap_or(0),
            tx_hash,
            height,
        })
    }
}

/// V2 WithdrawSubmit event from Terra bridge
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerraWithdrawSubmitEvent {
    /// Withdraw hash (32 bytes, base64)
    pub withdraw_hash: String,
    /// Source chain ID
    pub src_chain: ChainId,
    /// Token
    pub token: String,
    /// Amount
    pub amount: u128,
    /// Nonce
    pub nonce: u64,
    /// Transaction hash
    pub tx_hash: String,
    /// Block height
    pub height: u64,
}

impl TerraWithdrawSubmitEvent {
    /// Parse from WasmEvent
    pub fn from_wasm_event(event: &WasmEvent, tx_hash: String, height: u64) -> Option<Self> {
        if !event.is_action("withdraw_submit") {
            return None;
        }

        let src_chain_b64 = event.get("src_chain")?;
        let src_chain_bytes =
            base64::Engine::decode(&base64::engine::general_purpose::STANDARD, src_chain_b64)
                .ok()?;
        if src_chain_bytes.len() != 4 {
            return None;
        }
        let mut chain_bytes = [0u8; 4];
        chain_bytes.copy_from_slice(&src_chain_bytes);

        Some(TerraWithdrawSubmitEvent {
            withdraw_hash: event.get("withdraw_hash")?.clone(),
            src_chain: ChainId::from_bytes(chain_bytes),
            token: event.get("token")?.clone(),
            amount: event.get("amount")?.parse().ok()?,
            nonce: event.get("nonce")?.parse().ok()?,
            tx_hash,
            height,
        })
    }
}

/// V2 WithdrawApprove event from Terra bridge
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerraWithdrawApproveEvent {
    /// Withdraw hash (32 bytes, base64)
    pub withdraw_hash: String,
    /// Transaction hash
    pub tx_hash: String,
    /// Block height
    pub height: u64,
}

impl TerraWithdrawApproveEvent {
    /// Parse from WasmEvent
    pub fn from_wasm_event(event: &WasmEvent, tx_hash: String, height: u64) -> Option<Self> {
        if !event.is_action("withdraw_approve") {
            return None;
        }

        Some(TerraWithdrawApproveEvent {
            withdraw_hash: event.get("withdraw_hash")?.clone(),
            tx_hash,
            height,
        })
    }
}

/// V2 WithdrawCancel event from Terra bridge
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerraWithdrawCancelEvent {
    /// Withdraw hash (32 bytes, base64)
    pub withdraw_hash: String,
    /// Canceler address
    pub canceler: String,
    /// Transaction hash
    pub tx_hash: String,
    /// Block height
    pub height: u64,
}

impl TerraWithdrawCancelEvent {
    /// Parse from WasmEvent
    pub fn from_wasm_event(event: &WasmEvent, tx_hash: String, height: u64) -> Option<Self> {
        if !event.is_action("withdraw_cancel") {
            return None;
        }

        Some(TerraWithdrawCancelEvent {
            withdraw_hash: event.get("withdraw_hash")?.clone(),
            canceler: event.get("canceler")?.clone(),
            tx_hash,
            height,
        })
    }
}

/// V2 WithdrawExecute event from Terra bridge
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerraWithdrawExecuteEvent {
    /// Withdraw hash (32 bytes, base64)
    pub withdraw_hash: String,
    /// Recipient address
    pub recipient: String,
    /// Amount withdrawn
    pub amount: u128,
    /// Transaction hash
    pub tx_hash: String,
    /// Block height
    pub height: u64,
}

impl TerraWithdrawExecuteEvent {
    /// Parse from WasmEvent
    pub fn from_wasm_event(event: &WasmEvent, tx_hash: String, height: u64) -> Option<Self> {
        if !event.is_action("withdraw_execute") {
            return None;
        }

        Some(TerraWithdrawExecuteEvent {
            withdraw_hash: event.get("withdraw_hash")?.clone(),
            recipient: event.get("recipient")?.clone(),
            amount: event.get("amount")?.parse().ok()?,
            tx_hash,
            height,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wasm_event_parsing() {
        let tx_events = vec![TxEvent {
            event_type: "wasm".to_string(),
            attributes: vec![
                TxEventAttribute {
                    key: "_contract_address".to_string(),
                    value: "terra1...".to_string(),
                },
                TxEventAttribute {
                    key: "action".to_string(),
                    value: "deposit".to_string(),
                },
                TxEventAttribute {
                    key: "amount".to_string(),
                    value: "1000000".to_string(),
                },
            ],
        }];

        let wasm_events = WasmEvent::from_tx_events(&tx_events);
        assert_eq!(wasm_events.len(), 1);
        assert_eq!(wasm_events[0].action, "deposit");
        assert_eq!(wasm_events[0].get("amount"), Some(&"1000000".to_string()));
    }

    #[test]
    fn test_wasm_event_is_action() {
        let event = WasmEvent {
            contract_address: "terra1...".to_string(),
            action: "withdraw_approve".to_string(),
            attributes: HashMap::new(),
        };

        assert!(event.is_action("withdraw_approve"));
        assert!(!event.is_action("deposit"));
    }
}
