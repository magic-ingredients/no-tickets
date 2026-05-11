//! Canned event-type fixtures for the spike.
//!
//! The full CLI port (Task 4) will replace this with a real registry
//! client hitting the no-tickets-service /v1/registry/event-types
//! endpoint. For the spike scope (Task 2: validate the rmcp toolchain),
//! returning representative canned data is sufficient and keeps the
//! tests deterministic.

use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct EventTypeRow {
    pub id: String,
    pub domain: String,
    pub entity: String,
    pub action: String,
    pub version: String,
    #[serde(skip_serializing)]
    pub deprecated: bool,
}

impl EventTypeRow {
    fn new(domain: &str, entity: &str, action: &str, version: &str, deprecated: bool) -> Self {
        Self {
            id: format!("{domain}.{entity}.{action}.{version}"),
            domain: domain.to_string(),
            entity: entity.to_string(),
            action: action.to_string(),
            version: version.to_string(),
            deprecated,
        }
    }
}

pub fn all_event_types() -> Vec<EventTypeRow> {
    vec![
        EventTypeRow::new("auth", "session", "created", "v1", false),
        EventTypeRow::new("auth", "session", "revoked", "v1", false),
        EventTypeRow::new("billing", "invoice", "issued", "v2", false),
        EventTypeRow::new("billing", "invoice", "paid", "v2", false),
        // Older versions retained but marked deprecated — exercises the
        // `deprecated` filter in tests.
        EventTypeRow::new("billing", "invoice", "issued", "v1", true),
        EventTypeRow::new("registry", "event_type", "registered", "v1", true),
    ]
}
