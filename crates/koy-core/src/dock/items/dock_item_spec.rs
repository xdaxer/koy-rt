use super::dock_item_kind::DockItemKind;

/// Doğrulanmış dock öğesi. Yalnız geçerli türler, dolu kimlikler ve tekilleştirilmiş kayıtlar bu tipe girer.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DockItemSpec {
    pub kind: DockItemKind,
    pub id: String,
    pub command: String,
}

impl DockItemSpec {
    pub fn new(kind: DockItemKind, id: impl Into<String>, command: impl Into<String>) -> Self {
        DockItemSpec { kind, id: id.into(), command: command.into() }
    }
}
