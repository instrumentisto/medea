use std::io;

use actix::prelude::{Message, Recipient};
use actix_rt::spawn;

use futures::{Async, Future, Poll, Stream};

/// Different kinds of process signals
#[derive(PartialEq, Clone, Copy, Debug)]
pub enum SignalKind {
    /// SIGHUP
    Hup,
    /// SIGINT
    Int,
    /// SIGTERM
    Term,
    /// SIGQUIT
    Quit,
}

pub struct SignalMessage(pub SignalKind);

impl Message for SignalMessage {
    type Result = ();
}

pub struct SignalHandler {
    srv: Recipient<SignalMessage>,
    #[cfg(not(unix))]
    stream: SigStream,
    #[cfg(unix)]
    streams: Vec<SigStream>,
}

type SigStream = Box<Stream<Item = SignalKind, Error = io::Error>>;

impl SignalHandler {
    pub fn start(srv: Recipient<SignalMessage>) {
        let fut = {
            #[cfg(not(unix))]
            {
                tokio_signal::ctrl_c()
                    .map_err(|_| ())
                    .and_then(move |stream| Self {
                        srv,
                        stream: Box::new(stream.map(|_| SignalKind::Int)),
                    })
            }

            #[cfg(unix)]
            {
                use tokio_signal::unix;

                let mut sigs: Vec<
                    Box<Future<Item = SigStream, Error = io::Error>>,
                > = Vec::new();
                sigs.push(Box::new(
                    tokio_signal::unix::Signal::new(tokio_signal::unix::SIGINT)
                        .map(|stream| {
                            let s: SigStream =
                                Box::new(stream.map(|_| SignalKind::Int));
                            s
                        }),
                ));
                sigs.push(Box::new(
                    tokio_signal::unix::Signal::new(tokio_signal::unix::SIGHUP)
                        .map(|stream: unix::Signal| {
                            let s: SigStream =
                                Box::new(stream.map(|_| SignalKind::Hup));
                            s
                        }),
                ));
                sigs.push(Box::new(
                    tokio_signal::unix::Signal::new(
                        tokio_signal::unix::SIGTERM,
                    )
                    .map(|stream| {
                        let s: SigStream =
                            Box::new(stream.map(|_| SignalKind::Term));
                        s
                    }),
                ));
                sigs.push(Box::new(
                    tokio_signal::unix::Signal::new(
                        tokio_signal::unix::SIGQUIT,
                    )
                    .map(|stream| {
                        let s: SigStream =
                            Box::new(stream.map(|_| SignalKind::Quit));
                        s
                    }),
                ));
                futures_unordered(sigs)
                    .collect()
                    .map_err(|_| ())
                    .and_then(move |streams| SignalHandler { srv, streams })
            }
        };
        spawn(fut);
    }
}

impl Future for SignalHandler {
    type Error = ();
    type Item = ();

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        #[cfg(not(unix))]
        loop {
            match self.stream.poll() {
                Ok(Async::Ready(None)) | Err(_) => return Ok(Async::Ready(())),
                Ok(Async::Ready(Some(sig))) => {
                    let _ = self.srv.do_send(SignalMessage(sig));
                }
                Ok(Async::NotReady) => return Ok(Async::NotReady),
            };
        }
        #[cfg(unix)]
        {
            for s in &mut self.streams {
                loop {
                    match s.poll() {
                        Ok(Async::Ready(None)) | Err(_) => {
                            return Ok(Async::Ready(()))
                        }
                        Ok(Async::NotReady) => break,
                        Ok(Async::Ready(Some(sig))) => {
                            self.srv.do_send(SignalMessage(sig))
                        }
                    }
                }
            }
            Ok(Async::NotReady)
        }
    }
}
