use std::env;
use std::path::{PathBuf};

pub fn resolve_as_current_path(s: String) -> Option<PathBuf> {
    let result = env::current_exe();
    if result.is_ok() {
        let r = result.unwrap();
        return r.parent()
            .map(|v| v.join(s));
    } else {
        let e = result.err().unwrap();
        warn!("Get Current exe fail.err:{:?}",e);
    }
    None
}