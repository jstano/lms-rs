use crate::id_type;

id_type!(StandardSetId, uuid_v4);

pub struct StandardSet {
    id: StandardSetId
}

impl StandardSet {
    pub fn new(id: StandardSetId) -> Self {
        Self {
            id
        }
    }

    pub fn id(&self) -> StandardSetId {
        self.id
    }
}
