//! Accept loop + backend selection.
//!
//! Default backend: hyper 1.x. With `--features raw`: a hand-rolled HTTP/1.1
//! server (thread-per-connection, blocking). Both share the same router via
//! `route_ctx`, so protocol behavior is byte-identical; only the parse and
//! socket layers differ.

#[cfg(not(feature = "raw"))]
pub use hyper_backend::serve;

#[cfg(feature = "raw")]
pub use raw_backend::serve;

#[cfg(not(feature = "raw"))]
mod hyper_backend {
    use hyper::server::conn::http1;
    use hyper::service::service_fn;
    use hyper_util::rt::TokioIo;
    use std::convert::Infallible;
    use tokio::net::TcpListener;

    use crate::router::route;

    /// Serves HTTP/1.1 on `listener` forever: one spawned task per
    /// connection, keep-alive on, `TCP_NODELAY` on. Connection errors are
    /// intentionally silent (clients abort all the time; logging them would
    /// be noise).
    pub async fn serve(listener: TcpListener) {
        loop {
            let (stream, _peer) = match listener.accept().await {
                Ok(x) => x,
                Err(_) => continue,
            };
            let _ = stream.set_nodelay(true);
            let io = TokioIo::new(stream);
            tokio::spawn(async move {
                let svc = service_fn(|req| async move {
                    Ok::<_, Infallible>(route(req))
                });
                // writev merges the header and body writes into one syscall
                // per response, halving the socket work on the hot path.
                let _ = http1::Builder::new()
                    .writev(true)
                    .serve_connection(io, svc)
                    .await;
            });
        }
    }
}

#[cfg(feature = "raw")]
mod raw_backend {
    use tokio::net::TcpListener;

    /// The raw server is async on purpose: an earlier blocking-`std::net`
    /// version measured ~4x worse end-to-end latency than IOCP, because a
    /// blocked thread must be scheduled back in for every request while an
    /// overlapped read is completed by the kernel and picked up by an
    /// already-running worker. The parser/serializer here are still fully
    /// hand-rolled; only the IO model is shared with tokio.
    pub async fn serve(listener: TcpListener) {
        crate::raw::serve(listener).await;
    }
}