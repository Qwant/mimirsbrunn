use anyhow::anyhow;
use std::str::FromStr;

#[derive(Debug)]
pub(crate) enum Port {
    Tcp(u16),
    Udp(u16),
}

impl FromStr for Port {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let Some((port, protocol)) = s.split_once('/') else {
            return Err(anyhow!("malformed port binding: {s}"));
        };

        let port = port.parse::<u16>()?;
        match protocol {
            "tcp" => Ok(Port::Tcp(port)),
            "udp" => Ok(Port::Udp(port)),
            _ => Err(anyhow!("malformed port binding: {s}")),
        }
    }
}

impl ToString for Port {
    fn to_string(&self) -> String {
        match self {
            Port::Tcp(port) => format!("{port}/tcp"),
            Port::Udp(port) => format!("{port}/udp"),
        }
    }
}
