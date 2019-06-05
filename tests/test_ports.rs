//! Test ports generating.

/// Macro for generating ports numbers.
/// This should used when you want to start test server.
/// It's necessary because tests run asynchronously and ports
/// should not overlap. Enumerating start from 49151 because
/// based on [Registered by IANA ports][1] this is the last reserved port.
/// 16384 ought to be enough for anybody.
///
/// [1]: https://en.wikipedia.org/wiki/List_of_TCP_and_UDP_port_numbers
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
                        let port = 49151 + i as u16;
                        (t, port)
                    })
                    .collect()
            };
        }
    };
}

// Register your test by adding test name into this macro call.
generate_ports_for_tests!(three_members_p2p_video_call, pub_sub_video_call,);

/// Use it for easy get port by declared before name.
///
/// It will panic in runtime when port with provided name is not defined.
pub fn get_port_for_test(name: &str) -> u16 {
    *PORTS.get(name).expect("Port for this name is not defined!")
}
