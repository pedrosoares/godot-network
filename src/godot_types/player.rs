use godot::prelude::*;

#[derive(Debug, PartialEq, GodotClass, Clone)]
#[class(no_init, base=RefCounted)]
pub struct Player {
    #[var]
    pub id: i32,
    #[var]
    pub user_name: GString,
}

impl Player {
    pub fn to_godot(self) -> Gd<Self> {
        Gd::from_object(self)
    }
}
