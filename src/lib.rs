mod common;
mod config;
mod link;
mod proxy;

use std::sync::Arc;

use crate::config::Config;
use crate::link::generate_link;
use crate::proxy::RequestContext;

use proxy::parse_early_data;
use worker::*;

lazy_static::lazy_static! {
    static ref CONFIG: Arc<Config> = {
        let c = include_str!(env!("CONFIG_PATH"));
        Arc::new(Config::new(c))
    };
}

#[event(fetch)]
async fn main(req: Request, _: Env, _: Context) -> Result<Response> {
    match req.path().as_str() {
        "/deep_link_config" => link(req, CONFIG.clone()),
        path => match CONFIG.dispatch_inbound(path) {
            Some(inbound) => {
                let early_data = req.headers().get("sec-websocket-protocol")?;
                let early_data = parse_early_data(early_data)?;
                let context = RequestContext {
                    inbound,
                    request: Some(req),
                    ..Default::default()
                };
                tunnel(CONFIG.clone(), context, early_data.clone()).await
            }
            None => Response::empty(),
        },
    }
}

async fn tunnel(config: Arc<Config>, context: RequestContext, early_data: Option<Vec<u8>>) -> Result<Response> {
    let WebSocketPair { server, client } = WebSocketPair::new()?;

    server.accept()?;
    wasm_bindgen_futures::spawn_local(async move {
        let events = server.events().unwrap();

        if let Err(e) = proxy::process(config, context, &server, events, early_data).await {
            console_log!("[tunnel]: {}", e);
        }
    });

    Response::from_websocket(client)
}

fn link(req: Request, config: Arc<Config>) -> Result<Response> {
    let host = req.url()?.host().map(|x| x.to_string()).unwrap_or_default();
    Response::from_json(&generate_link(&config, &host))
}
