//! Cap Redis wire bytes per short-lived operation, before response decoding.
use std::{
    io,
    pin::Pin,
    task::{Context, Poll},
};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

pub trait Transport: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send> Transport for T {}

pub struct BoundedStream {
    inner: Box<dyn Transport>,
    remaining: usize,
}
impl BoundedStream {
    pub fn new(inner: Box<dyn Transport>, limit: usize) -> Self {
        Self {
            inner,
            remaining: limit,
        }
    }
}
impl AsyncRead for BoundedStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        out: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        if out.remaining() == 0 {
            return Poll::Ready(Ok(()));
        }
        if self.remaining == 0 {
            return Poll::Ready(Err(io::Error::other("Redis response exceeded wire budget")));
        }
        let limit = out.remaining().min(self.remaining);
        let mut limited = ReadBuf::new(out.initialize_unfilled_to(limit));
        match Pin::new(&mut self.inner).poll_read(cx, &mut limited) {
            Poll::Ready(Ok(())) => {
                let read = limited.filled().len();
                out.advance(read);
                self.remaining -= read;
                Poll::Ready(Ok(()))
            }
            other => other,
        }
    }
}
impl AsyncWrite for BoundedStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        data: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.inner).poll_write(cx, data)
    }
    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }
    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    #[tokio::test]
    async fn rejects_response_bytes_beyond_budget() {
        let (client, mut server) = tokio::io::duplex(64);
        server.write_all(b"0123456789").await.unwrap();
        let mut stream = BoundedStream::new(Box::new(client), 5);
        let mut bytes = [0; 5];
        stream.read_exact(&mut bytes).await.unwrap();
        assert_eq!(&bytes, b"01234");
        assert!(stream.read_u8().await.is_err());
    }
}
