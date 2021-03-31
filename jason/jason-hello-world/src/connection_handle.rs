use dart_sys::Dart_Handle;

pub struct ConnectionHandle;

impl ConnectionHandle {
    pub fn get_remote_member_id(&self) -> String {
        "foobar".to_string()
    }
}
