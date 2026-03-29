# Import Policies

When clients transfer between rooms, their data passes through customs.

## The Problem

A client brings a passport: `{ role: "admin", items: ["GodSword", "AdminKey"], credits: 999999 }`

Do you trust it?

## The Solution

Each authority defines an **Import Policy** that validates and sanitizes incoming client data.

## Policy Definition

```rust
struct ImportPolicy {
    // Identity
    preserve_identity: bool,

    // Capabilities and roles
    allowed_roles: HashSet<RoleId>,
    banned_capabilities: HashSet<CapabilityId>,

    // Carried items or assets (application-defined)
    allowed_items: HashSet<ItemId>,
    banned_items: HashSet<ItemId>,
    max_carried_items: usize,

    // Numeric fields: clamp to local limits
    field_limits: HashMap<String, RangeLimit>,
}
```

## Validation Flow

```rust
fn validate_passport(passport: Passport, policy: &ImportPolicy) -> ValidatedClient {
    let mut client = ValidatedClient::new();

    // Numeric fields: clamp to policy limits
    for (field, limit) in &policy.field_limits {
        if let Some(value) = passport.get_field(field) {
            client.set_field(field, value.clamp(limit.min, limit.max));
        }
    }

    // Items: filter against allowlist/blocklist
    for item in passport.items.iter().take(policy.max_carried_items) {
        if policy.banned_items.contains(&item.id) {
            client.add_notification(format!("Banned item confiscated: {}", item.name));
            continue;
        }

        if policy.allowed_items.is_empty() || policy.allowed_items.contains(&item.id) {
            client.carry.push(item.clone());
        } else {
            client.add_notification(format!("Item not recognized: {}", item.name));
        }
    }

    client
}
```

## Policy Examples

### Open Room (Permissive)

Accepts most incoming data, blocks known contraband:

```toml
[import_policy]
allowed_items = "*"  # All items allowed
banned_items = ["debug_tool", "admin_key"]
```

### Restricted Room (Filtered)

Accepts only specific capabilities and items:

```toml
[import_policy]
allowed_roles = ["member"]
allowed_items = ["avatar", "display_name"]
max_carried_items = 5
```

For a game, this might be a PvP arena that only allows basic gear. For a social room, this might be a community that imports your identity but ignores reputation from other platforms.

### Fresh-Start Room

Accepts identity but ignores all other incoming state:

```toml
[import_policy]
preserve_identity = true  # Keep name/appearance
reset_stats = true        # Ignore incoming numeric fields
reset_inventory = true    # Ignore incoming items
```

For a game, this is a tutorial zone or permadeath server. For a professional network, this is a space that accepts your identity but not your social history from other communities.

## Notifications

When data is adjusted or items confiscated, clients receive notifications:

- "Your 'credits' field was adjusted from 999999 to 1000 (server limit)"
- "Item 'GodSword' is not allowed in this room"
- "Contraband detected: 'AdminKey' confiscated"

Transparency builds trust. Never silently drop data.
