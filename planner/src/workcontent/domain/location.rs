use crate::id_type;

id_type!(LocationId, uuid_v4);

pub struct Location {
    id: LocationId,
}

impl Location {
    pub fn new(id: LocationId) -> Self {
        Self { id }
    }

    pub fn id(&self) -> LocationId {
        self.id
    }
}
