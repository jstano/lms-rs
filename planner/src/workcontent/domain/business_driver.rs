use crate::id_type;
use crate::workcontent::domain::location::LocationId;

id_type!(BusinessDriverId, uuid_v4);

pub struct BusinessDriver {
    id: BusinessDriverId,
    location_id: LocationId,
}

impl BusinessDriver {
    pub fn new(id: BusinessDriverId, location_id: LocationId) -> Self {
        Self {
            id,
            location_id
        }
    }

    pub fn id(&self) -> BusinessDriverId {
        self.id
    }

    pub fn location_id(&self) -> LocationId {
        self.location_id
    }
}
