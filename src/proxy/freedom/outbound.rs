use crate::proxy::{Proxy, RequestContext};

use std::pin::Pin;
use std::task::{Context, Poll};

use async_trait::async_trait;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use worker::*;

pub struct FreedomStream {
    stream: Socket,
}

impl FreedomStream {
    pub fn new(_context: RequestContext, stream: Socket) -> Self {
        FreedomStream {
            stream,
        }
    }
}

#[async_trait]
impl Proxy for FreedomStream {
    async fn process(&mut self) -> Result<()> {
        Ok(())
    }
}

impl AsyncRead for FreedomStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<tokio::io::Result<()>> {
        Pin::new(&mut self.stream).poll_read(cx, buf)
    }
}

impl AsyncWrite for FreedomStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context,
        buf: &[u8],
    ) -> Poll<tokio::io::Result<usize>> {
        Pin::new(&mut self.stream).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<tokio::io::Result<()>> {
        Pin::new(&mut self.stream).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<tokio::io::Result<()>> {
        Pin::new(&mut self.stream).poll_shutdown(cx)
    }
}
