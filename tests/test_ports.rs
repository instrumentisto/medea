//! Test ports generating.

/// Macro for generating ports numbers.
/// This should used when you want to start test server.
/// It's necessary because tests run asynchronously and ports
/// should not overlap. Enumerating start from 40000 because
/// the chance to cross the already used port is very small.
macro_rules! generate_ports_for_tests {
    ( $( $x:tt ),* $(,)* ) => {
        use lazy_static::lazy_static;
        use hashbrown::HashMap;

        lazy_static! {
            static ref PORTS: HashMap<String, u16> = {
                let mut names: Vec<String> = Vec::new();
                $( names.push(stringify!($x).to_string()); )*
                names
                    .into_iter()
                    .enumerate()
                    .map(|(i, t)| {
                        let port = 40000 + i as u16;
                        (t, port)
                    })
                    .collect()
            };
        }
    };
}

generate_ports_for_tests!(
    should_work_three_members_p2p_video_call,
    should_work_pub_sub_video_call,
);

/// Use it for easy get port by declared before name.
///
/// It will panic in runtime when port with provided name is not defined.
pub fn get_port_for_test(name: &str) -> u16 {
    *PORTS.get(name).expect("Port for this name is not defined!")
}
