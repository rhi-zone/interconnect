//! Game protocol types.

use interconnect_core::Identity;
use serde::{Deserialize, Serialize};

/// Player intent (what the client wants to do).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum GameIntent {
    /// Move in a direction.
    Move { dx: f32, dy: f32 },
    /// Use an item from inventory.
    UseItem { slot: usize },
    /// Pick up an item from the world.
    PickUp { item_id: u64 },
    /// Drop an item.
    Drop { slot: usize },
    /// Request transfer to another zone.
    Transfer { destination: String },
}

/// World snapshot (authoritative state).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameSnapshot {
    pub tick: u64,
    pub players: Vec<PlayerState>,
    pub items: Vec<WorldItem>,
    pub zone_name: String,
}

/// A player's visible state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerState {
    pub identity: Identity,
    pub name: String,
    pub x: f32,
    pub y: f32,
    pub health: u32,
    pub equipped: Option<ItemKind>,
}

/// An item in the world (not in inventory).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldItem {
    pub id: u64,
    pub kind: ItemKind,
    pub x: f32,
    pub y: f32,
}

/// Item types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemKind {
    Sword,
    Shield,
    Potion,
    Key,
    Gem,
    Torch,
}

impl ItemKind {
    pub fn is_weapon(&self) -> bool {
        matches!(self, ItemKind::Sword)
    }
}

/// Inventory item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InventoryItem {
    pub kind: ItemKind,
    pub count: u32,
}

/// Passport for zone transfer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GamePassport {
    pub identity: Identity,
    pub name: String,
    pub health: u32,
    pub max_health: u32,
    pub inventory: Vec<InventoryItem>,
    pub origin_zone: String,
}

impl GamePassport {
    pub fn to_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap()
    }

    pub fn from_bytes(data: &[u8]) -> Option<Self> {
        serde_json::from_slice(data).ok()
    }
}

/// Import policy result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportResult {
    pub accepted_items: Vec<InventoryItem>,
    pub rejected_items: Vec<(InventoryItem, String)>,
    pub health: u32,
}
