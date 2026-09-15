use crate::id_type;

id_type!(EnvironmentId, uuid_v4);

/// An operating condition a property distinguishes its standards by — a
/// seasonal mode, a service level, and so on.
///
/// Standards that vary by environment hold one value per environment and pick
/// the one matching the date being planned. Only the identity is modelled here:
/// resolving which environment a date falls in is a separate concern that the
/// flowed generator takes as given.
pub struct Environment {
    id: EnvironmentId,
    name: String,
}

impl Environment {
    pub fn new(id: EnvironmentId, name: String) -> Self {
        Self { id, name }
    }

    pub fn id(&self) -> EnvironmentId {
        self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}
