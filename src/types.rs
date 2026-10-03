use godot::prelude::*;

#[derive(Debug, Clone, PartialEq, GodotClass)]
#[class(no_init, base=RefCounted)]
pub struct Player {
    #[var]
    pub id: i32,
    #[var]
    pub user_name: GString,
}

impl Player {
    pub fn new(id: i32, user_name: &str) -> Gd<Self> {
        Gd::from_object(Self {
            id,
            user_name: user_name.into(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, GodotClass)]
#[class(no_init, base=RefCounted)]
pub struct Match {
    #[var]
    pub id: i32,
    /// -1 when unknown (rooms in a `MatchList`).
    #[var]
    pub owner_id: i32,
    #[var]
    pub name: GString,
    #[var]
    pub number_of_players: i32,
}

impl Match {
    pub fn new(id: i32, owner_id: i32, name: &str, number_of_players: i32) -> Gd<Self> {
        Gd::from_object(Self {
            id,
            owner_id,
            name: name.into(),
            number_of_players,
        })
    }
}
