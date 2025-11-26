use godot::prelude::*;

#[derive(Debug, PartialEq, GodotClass)]
#[class(no_init, base=RefCounted)]
pub struct Match {
    #[var]
    pub id: i32,
    #[var]
    pub owner_id: i32,
    #[var]
    pub name: GString,
    #[var]
    pub number_of_players: i32,
}

impl Match {
    pub fn to_godot(self) -> Gd<Self> {
        Gd::from_object(self)
    }
}
