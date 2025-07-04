pub mod bepass;
pub mod freedom;
pub mod blackhole;
pub mod relay;
pub mod trojan;
pub mod vless;
pub mod vmess;
pub mod ws;
pub mod mock_udp;

use std::sync::Arc;

use crate::config::*;
use ws::WebSocketStream;
use base64::{Engine as _, engine::{self}};

use async_trait::async_trait;
use tokio::io::{AsyncRead, AsyncWrite};
use worker::*;

#[async_trait]
pub trait Proxy: AsyncRead + AsyncWrite + Unpin + Send {
    async fn process(&mut self) -> Result<()>;
}

#[async_trait]
impl Proxy for Socket {
    async fn process(&mut self) -> Result<()> {
        Ok(())
    }
}

#[derive(Default, Debug, Clone)]
pub enum Network {
    #[default]
    Tcp,
    Udp,
}

impl Network {
    fn from_str(s: &str) -> Result<Self> {
        match s {
            "tcp" => Ok(Self::Tcp),
            "udp" => Ok(Self::Udp),
            _ => Err(Error::RustError("invalid network type".to_string())),
        }
    }

    fn from_byte(b: u8) -> Result<Self> {
        match b {
            0x01 => Ok(Self::Tcp),
            0x02 => Ok(Self::Udp),
            _ => Err(Error::RustError("invalid network type".to_string())),
        }
    }

    fn to_byte(&self) -> u8 {
        match self {
            Self::Tcp => 0x01,
            Self::Udp => 0x02,
        }
    }
}

#[derive(Default)]
pub struct RequestContext {
    pub address: String,
    pub port: u16,
    pub network: Network,
    pub inbound: Inbound,
    pub request: Option<Request>,
}

unsafe impl Send for RequestContext {}

impl Clone for RequestContext {
    fn clone(&self) -> Self {
        let port = self.port;
        let address = self.address.clone();
        let network = self.network.clone();
        let inbound = self.inbound.clone();

        Self {
            address,
            port,
            network,
            inbound,
            // to avoid unnecessary overheads of copying:
            // context is getting filled during processing a request
            // so no need to clone any data here
            request: None,
        }
    }
}

async fn connect_outbound(ctx: RequestContext, outbound: Outbound) -> Result<Box<dyn Proxy>> {
    // Detect the DNS request
    if matches!(ctx.network, Network::Udp) && ctx.port == 53 {
        console_log!("[MockUDP] Detected UDP DNS request, will use DoH to handle it");
        return Ok(Box::new(mock_udp::outbound::MockUDPStream::new()));
    }

    let (addr, port) = match outbound.protocol {
        Protocol::Freedom => (&ctx.address, ctx.port),
        _ => {
            let address = if outbound.addresses.len() > 0 {
                &outbound.addresses[fastrand::usize(..outbound.addresses.len())]
            } else {
                &ctx.address
            };

            (address, outbound.port)
        }
    };

    console_log!(
        "[{:?}] connecting to upstream {addr}:{port}",
        outbound.protocol
    );

    let socket = Socket::builder().connect(addr, port)?;

    let mut stream: Box<dyn Proxy> = match outbound.protocol {
        Protocol::MockUDP => Box::new(mock_udp::outbound::MockUDPStream::new()),
        Protocol::Vless => Box::new(vless::outbound::VlessStream::new(ctx, outbound, socket)),
        Protocol::Trojan => Box::new(trojan::outbound::TrojanStream::new(ctx, outbound, socket)),
        Protocol::RelayV1 => Box::new(relay::outbound::RelayStream::new(
            ctx,
            socket,
            relay::outbound::RelayVersion::V1,
        )),
        Protocol::RelayV2 => Box::new(relay::outbound::RelayStream::new(
            ctx,
            socket,
            relay::outbound::RelayVersion::V2,
        )),
        Protocol::Blackhole => Box::new(blackhole::outbound::BlackholeStream),
        _ => Box::new(freedom::outbound::FreedomStream::new(ctx, socket)),
    };

    stream.process().await?;
    Ok(stream)
}

pub async fn process(
    config: Arc<Config>,
    context: RequestContext,
    ws: &WebSocket,
    events: EventStream<'_>,
    early_data: Option<Vec<u8>>
) -> Result<()> {

    let ws = WebSocketStream::new(events, ws, early_data);
    match context.inbound.protocol {
        Protocol::Vmess => {
            vmess::inbound::VmessStream::new(config, context, ws)
                .process()
                .await
        }
        Protocol::Vless => {
            vless::inbound::VlessStream::new(config, context, ws)
                .process()
                .await
        }
        Protocol::Trojan => {
            trojan::inbound::TrojanStream::new(config, context, ws)
                .process()
                .await
        }
        Protocol::Bepass => {
            bepass::inbound::BepassStream::new(config, context, ws)
                .process()
                .await
        }
        _ => return Err(Error::RustError("invalid inbound protocol".to_string())),
    }
}

pub fn parse_early_data(data: Option<String>) -> Result<Option<Vec<u8>>> {
    if let Some(data) = data {
        if !data.is_empty() {
            let s = data.replace('+', "-").replace('/', "_").replace("=", "");
            match engine::general_purpose::URL_SAFE_NO_PAD.decode(s) {
                Ok(early_data) => return Ok(Some(early_data)),
                Err(err) => {
                    console_error!("Error while parsing early data: {err})");
                    return Err(Error::RustError(format!("Error while parsing early data: {err}")))
                },
            }
        }
    }
    Ok(None)
}