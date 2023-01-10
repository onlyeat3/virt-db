use sea_orm::ActiveValue::Set;
use sea_orm::{ActiveValue, NotSet, Value};

pub fn option_to_active_value<V: Into<Value>>(option: Option<V>) -> ActiveValue<V> {
    match option {
        None => NotSet,
        Some(v) => Set(v),
    }
}
