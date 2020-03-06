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

#[derive(Debug)]
pub struct UnknownProtocol;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Protocol {
    Udp,
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

const TURN_SESSION_ID_FACTOR: u64 = 1_000_000_000_000_000;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionId(pub u64);

impl SessionId {
    pub fn server_id(self) -> u64 {
        self.0 / TURN_SESSION_ID_FACTOR
    }

    pub fn session_id(self) -> u64 {
        self.0 - (self.server_id() * TURN_SESSION_ID_FACTOR)
    }
}

impl fmt::Display for SessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(f, "{:018}", self.0)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Session {
    pub num: u32,
    pub id: SessionId,
    pub user: String,
    pub realm: String,
    pub started: Duration,
    pub expiring_in: Duration,
    pub client_protocol: Protocol,
    pub relay_protocol: Protocol,
    pub client_addr: String,
    pub server_addr: String,
    pub relay_addr: String,
    pub fingreprints_enforced: bool,
    pub mobile: bool,
    pub traffic_usage: TrafficUsage,
    pub rate_sent: String,
    pub rate_receive: String,
    pub total_rate: String,
    pub peers: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrafficUsage {
    pub received_packets: u64,
    pub received_bytes: u64,
    pub sent_packets: u64,
    pub sent_bytes: u64,
}

impl TrafficUsage {
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
    let (input, rate_sent) = coturn_stat(input, "s=")?;
    let (input, total_rate) =
        delimited(tag("total="), digit1, tag(" (bytes per sec)"))(input)?;

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
        rate_receive: rate_receive.to_string(),
        rate_sent: rate_sent.to_string(),
        total_rate: total_rate.to_string(),
        peers: peers.into_iter().map(ToString::to_string).collect(),
    };

    Ok((input, session))
}

fn coturn_bool_to_bool(input: &str) -> bool {
    input == "ON"
}

fn coturn_field_name<'a>(
    input: &'a str,
    field_name: &str,
) -> IResult<&'a str, &'a str> {
    delimited(many0(one_of(" \n\t\r,:")), tag(field_name), multispace0)(input)
}

fn coturn_stat<'a>(
    input: &'a str,
    predicate: &str,
) -> IResult<&'a str, &'a str> {
    delimited(tag(predicate), digit1, many0(one_of(" \n\t\r,")))(input)
}

fn address<'a>(input: &'a str) -> IResult<&'a str, &'a str> {
    is_a("1234567890.:[]")(input)
}

pub fn parse_sessions(input: &str) -> IResult<&str, Vec<Session>> {
    many0(parse_session)(input)
}
