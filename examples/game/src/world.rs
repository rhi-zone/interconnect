//! World state and simulation.

use crate::protocol::{
    GamePassport, GameSnapshot, ImportResult, InventoryItem, ItemKind, PlayerState, WorldItem,
};
use interconnect_core::Identity;
use std::collections::HashMap;

/// A player in the world.
pub struct Player {
    pub identity: Identity,
    pub name: String,
    pub x: f32,
    pub y: f32,
    pub health: u32,
    pub max_health: u32,
    pub inventory: Vec<InventoryItem>,
}

impl Player {
    pub fn new(identity: Identity, name: String) -> Self {
        Self {
            identity,
            name,
            x: 0.0,
            y: 0.0,
            health: 100,
            max_health: 100,
            inventory: Vec::new(),
        }
    }

    pub fn from_passport(passport: GamePassport, import: ImportResult) -> Self {
        Self {
            identity: passport.identity,
            name: passport.name,
            x: 0.0, // Spawn at origin
            y: 0.0,
            health: import.health,
            max_health: passport.max_health,
            inventory: import.accepted_items,
        }
    }

    pub fn to_passport(&self, origin_zone: String) -> GamePassport {
        GamePassport {
            identity: self.identity.clone(),
            name: self.name.clone(),
            health: self.health,
            max_health: self.max_health,
            inventory: self.inventory.clone(),
            origin_zone,
        }
    }

    pub fn to_state(&self) -> PlayerState {
        PlayerState {
            identity: self.identity.clone(),
            name: self.name.clone(),
            x: self.x,
            y: self.y,
            health: self.health,
            equipped: self.inventory.first().map(|i| i.kind),
        }
    }

    pub fn move_by(&mut self, dx: f32, dy: f32) {
        self.x = (self.x + dx).clamp(-100.0, 100.0);
        self.y = (self.y + dy).clamp(-100.0, 100.0);
    }
}

/// World state.
pub struct World {
    pub name: String,
    pub tick: u64,
    pub players: HashMap<Identity, Player>,
    pub items: Vec<WorldItem>,
    pub allow_weapons: bool,
    next_item_id: u64,
}

impl World {
    pub fn new(name: String) -> Self {
        // "Cave" zones don't allow weapons
        let allow_weapons = !name.to_lowercase().contains("cave");

        let mut world = Self {
            name,
            tick: 0,
            players: HashMap::new(),
            items: Vec::new(),
            allow_weapons,
            next_item_id: 1,
        };

        // Spawn some items
        world.spawn_item(ItemKind::Potion, 5.0, 5.0);
        world.spawn_item(ItemKind::Sword, -5.0, 3.0);
        world.spawn_item(ItemKind::Torch, 0.0, -5.0);
        world.spawn_item(ItemKind::Gem, 10.0, 10.0);

        world
    }

    fn spawn_item(&mut self, kind: ItemKind, x: f32, y: f32) {
        self.items.push(WorldItem {
            id: self.next_item_id,
            kind,
            x,
            y,
        });
        self.next_item_id += 1;
    }

    /// Apply import policy to incoming passport.
    pub fn apply_import_policy(&self, passport: &GamePassport) -> ImportResult {
        let mut accepted = Vec::new();
        let mut rejected = Vec::new();

        for item in &passport.inventory {
            if !self.allow_weapons && item.kind.is_weapon() {
                rejected.push((item.clone(), "Weapons not allowed in this zone".to_string()));
            } else {
                accepted.push(item.clone());
            }
        }

        // Clamp health to reasonable bounds
        let health = passport.health.clamp(1, 100);

        ImportResult {
            accepted_items: accepted,
            rejected_items: rejected,
            health,
        }
    }

    pub fn add_player(&mut self, player: Player) {
        self.players.insert(player.identity.clone(), player);
    }

    pub fn remove_player(&mut self, identity: &Identity) -> Option<Player> {
        self.players.remove(identity)
    }

    pub fn tick(&mut self) {
        self.tick += 1;
        // Could add physics, AI, etc. here
    }

    pub fn snapshot(&self) -> GameSnapshot {
        GameSnapshot {
            tick: self.tick,
            players: self.players.values().map(|p| p.to_state()).collect(),
            items: self.items.clone(),
            zone_name: self.name.clone(),
        }
    }
}
