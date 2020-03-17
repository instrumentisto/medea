//! Parser of Coturn's `ps` (print sessions) output with [`nom`].

use std::{convert::TryFrom, fmt, time::Duration};

use nom::{
    bytes::complete::{is_a, is_not, tag},
    character::complete::{
        alpha1, alphanumeric1, char, digit1, multispace0, one_of, space0,
    },
    multi::{many0, many1},
    sequence::{delimited, terminated},
    IResult,
};

/// Unknown protocol found.
#[derive(Clone, Copy, Debug)]
pub struct UnknownProtocol;

/// All known protocols which can be in a Coturn output.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Protocol {
    /// [UDP (User Datagram Protocol)][1]
    ///
    /// [1]: https://en.wikipedia.org/wiki/User_Datagram_Protocol
    Udp,

    /// [TCP (Transmission Control Protocol)][2]
    ///
    /// [2]: https://en.wikipedia.org/wiki/Transmission_Control_Protocol
    Tcp,
}

impl TryFrom<&str> for Protocol {
    type Error = UnknownProtocol;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "TCP" => Ok(Protocol::Tcp),
            "UDP" => Ok(Protocol::Udp),
            _ => Err(UnknownProtocol),
        }
    }
}

impl fmt::Display for Protocol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Protocol::Udp => write!(f, "UDP"),
            Protocol::Tcp => write!(f, "TCP"),
        }
    }
}

/// Coturns's sessions ID's factor.
///
/// Sessions ID of Coturn's sessions contains server ID and session ID in this
/// server's IDs scope. This data is stored with this algorithm on Coturn side:
///
/// `server_id * TURN_SESSION_ID_FACTOR + session_id`
///
/// [This constant in the Coturn source code][1]
///
/// [1]: https://tinyurl.com/tlt77bm
const TURN_SESSION_ID_FACTOR: u64 = 1_000_000_000_000_000;

/// ID of [`Session`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SessionId(pub u64);

impl SessionId {
    /// Extracts Coturn server ID from this [`SessionId`].
    pub fn server_id(self) -> u64 {
        self.0 / TURN_SESSION_ID_FACTOR
    }

    /// Extracts Coturn session ID from this [`SessionId`].
    pub fn session_id(self) -> u64 {
        self.0 - (self.server_id() * TURN_SESSION_ID_FACTOR)
    }
}

impl fmt::Display for SessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(f, "{:018}", self.0)
    }
}

/// Coturn's session information.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Session {
    /// Num of the [`Session`] in a Coturn's output.
    pub num: u32,

    /// ID of this [`Session`].
    pub id: SessionId,

    /// Username of this [`Session`].
    pub user: String,

    /// Realm where this [`Session`] is exists.
    pub realm: String,

    /// Time when this [`Session`] was started.
    pub started: Duration,

    /// Time after which this [`Session`] will expire.
    ///
    /// This time may change.
    pub expiring_in: Duration,

    /// Protocol of client.
    pub client_protocol: Protocol,

    /// Protocol of relay.
    pub relay_protocol: Protocol,

    /// Client's address.
    pub client_addr: String,

    /// Server's address.
    pub server_addr: String,

    /// Relays's address.
    pub relay_addr: String,

    /// `true` if fingreprints is enforced for this [`Session`].
    pub fingreprints_enforced: bool,

    /// `true` if this is mobile [`Session`].
    pub mobile: bool,

    /// Traffic usage of this [`Session`].
    pub traffic_usage: TrafficUsage,

    /// Sent rate of this [`Session`] (in bytes).
    pub rate_sent: u64,

    /// Receive rate of this [`Session`] (in bytes).
    pub rate_receive: u64,

    /// Total rate of this [`Session`] (in bytes)
    ///
    /// `rate_sent + rate_receive`
    pub total_rate: u64,

    /// Peer addresses.
    pub peers: Vec<String>,
}

/// Traffic usage of the [`Session`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TrafficUsage {
    /// Count of received packets.
    pub received_packets: u64,

    /// Count of received bytes.
    pub received_bytes: u64,

    /// Count of sent packets.
    pub sent_packets: u64,

    /// Count of sent bytes.
    pub sent_bytes: u64,
}

impl TrafficUsage {
    /// Tries to parse [`Session`]'s [`TrafficUsage`] from a provided [`str`].
    pub fn parse(input: &str) -> IResult<&str, Self> {
        let (input, rp) = coturn_stat(input, "rp=")?;
        let received_packets = rp.parse().unwrap();
        let (input, rb) = coturn_stat(input, "rb=")?;
        let received_bytes = rb.parse().unwrap();
        let (input, sp) = coturn_stat(input, "sp=")?;
        let sent_packets = sp.parse().unwrap();
        let (input, sb) = coturn_stat(input, "sb=")?;
        let sent_bytes = sb.parse().unwrap();

        Ok((
            input,
            Self {
                received_packets,
                received_bytes,
                sent_packets,
                sent_bytes,
            },
        ))
    }
}

/// Tries to parse one session from a provided [`str`].
fn parse_session(input: &str) -> IResult<&str, Session> {
    let (input, num) =
        delimited(many0(one_of(" \n\r,")), digit1, char(')'))(input)?;
    let num: u32 = num.parse().unwrap();

    let (input, _) = coturn_field_name(input, "id=")?;
    let (input, id) = digit1(input)?;
    let id: u64 = id.parse().unwrap();

    let (input, _) = coturn_field_name(input, "user")?;
    let (input, user) = delimited(char('<'), is_not(">"), char('>'))(input)?;

    let (input, _) = coturn_field_name(input, "realm:")?;
    let (input, realm) = alphanumeric1(input)?;

    let (input, _) = coturn_field_name(input, "started")?;
    let (input, started) = terminated(digit1, tag(" secs ago"))(input)?;
    let started: u64 = started.parse().unwrap();

    let (input, _) = coturn_field_name(input, "expiring in")?;
    let (input, expiring_in) = terminated(digit1, tag(" secs"))(input)?;
    let expiring_in: u64 = expiring_in.parse().unwrap();

    let (input, _) = coturn_field_name(input, "client protocol")?;
    let (input, client_protocol) = alpha1(input)?;

    let (input, _) = coturn_field_name(input, "relay protocol")?;
    let (input, relay_protocol) = alpha1(input)?;

    let (input, _) = coturn_field_name(input, "client addr")?;
    let (input, client_addr) = address(input)?;

    let (input, _) = coturn_field_name(input, "server addr")?;
    let (input, server_addr) = address(input)?;

    let (input, _) = coturn_field_name(input, "relay addr")?;
    let (input, relay_addr) = address(input)?;

    let (input, _) = coturn_field_name(input, "fingerprints enforced:")?;
    let (input, fingreprints_enforced) = alpha1(input)?;

    let (input, _) = coturn_field_name(input, "mobile:")?;
    let (input, mobile) = alpha1(input)?;

    let (input, _) = coturn_field_name(input, "usage:")?;
    let (input, traffic_usage) = TrafficUsage::parse(input)?;

    let (input, _) = coturn_field_name(input, "rate:")?;
    let (input, rate_receive) = coturn_stat(input, "r=")?;
    let rate_receive: u64 = rate_receive.parse().unwrap();
    let (input, rate_sent) = coturn_stat(input, "s=")?;
    let rate_sent: u64 = rate_sent.parse().unwrap();
    let (input, total_rate) =
        delimited(tag("total="), digit1, tag(" (bytes per sec)"))(input)?;
    let total_rate: u64 = total_rate.parse().unwrap();

    let (input, peers) = match coturn_field_name(input, "peers:") {
        Ok((input, _)) => {
            let (input, peers) =
                many1(delimited(space0, address, tag("\r\n")))(input)?;
            (input, peers)
        }
        Err(_) => (input, Vec::new()),
    };

    let session = Session {
        num,
        id: SessionId(id),
        user: user.to_string(),
        realm: realm.to_string(),
        started: Duration::from_secs(started),
        expiring_in: Duration::from_secs(expiring_in),
        client_protocol: Protocol::try_from(client_protocol).unwrap(),
        relay_protocol: Protocol::try_from(relay_protocol).unwrap(),
        client_addr: client_addr.to_string(),
        server_addr: server_addr.to_string(),
        relay_addr: relay_addr.to_string(),
        fingreprints_enforced: coturn_bool_to_bool(fingreprints_enforced),
        mobile: coturn_bool_to_bool(mobile),
        traffic_usage,
        rate_receive,
        rate_sent,
        total_rate,
        peers: peers.into_iter().map(ToString::to_string).collect(),
    };

    Ok((input, session))
}

/// Checks that provided [`str`] is `ON` which is analog of `true` in the Coturn
/// stats.
fn coturn_bool_to_bool(input: &str) -> bool {
    input == "ON"
}

/// Tries to parse Coturn's field name.
fn coturn_field_name<'a>(
    input: &'a str,
    field_name: &str,
) -> IResult<&'a str, &'a str> {
    delimited(many0(one_of(" \n\t\r,:")), tag(field_name), multispace0)(input)
}

/// Tries to parse coturn traffic related stat.
fn coturn_stat<'a>(
    input: &'a str,
    predicate: &str,
) -> IResult<&'a str, &'a str> {
    delimited(tag(predicate), digit1, many0(one_of(" \n\t\r,")))(input)
}

/// Tries to parse address.
fn address<'a>(input: &'a str) -> IResult<&'a str, &'a str> {
    is_a("1234567890.:[]")(input)
}

/// Tries to parse all [`Session`]s from a provided [`str`].
pub fn parse_sessions(input: &str) -> IResult<&str, Vec<Session>> {
    many0(parse_session)(input)
}
