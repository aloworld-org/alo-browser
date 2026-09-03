//! TLS, rented.
//!
//! **This is the only file that names `rustls`.** ADR 0001 says to rent the
//! physics and names TLS among it; ADR 0005 says the same thing from the other
//! side, listing TLS first among the rented code whose `unsafe` is not ours to
//! remove and which is therefore one of the four reasons for the sandbox.
//! Nobody writes their own TLS, and not because they are afraid of work.
//!
//! # The provider, and what it costs
//!
//! `ring` rather than `aws-lc-rs`, which is `rustls`'s default. Both carry C;
//! there is no pure-Rust provider mature enough to put a browser's traffic
//! through. `ring` is the smaller of the two and the more widely audited, and
//! it builds without a C toolchain's worth of configuration. **That C is
//! exactly what ADR 0005's second reason is about**, and it is why a renderer
//! never speaks TLS: the handshake happens in the browser process, and the
//! renderer is handed bytes.
//!
//! # What is ours
//!
//! [`crate::certificate`] — what a person is told when a certificate is
//! refused. The chain validation is `rustls`'s; the sentence is ours, and it
//! is the whole of the security value at this layer, because every browser
//! that got the sentence wrong has a generation of users trained to click
//! through it.
//!
//! # There is no way to turn verification off
//!
//! Not a flag, not a constructor, not a feature. `rustls` makes it possible —
//! a `ServerCertVerifier` that returns `Ok` — and this file does not do it and
//! does not expose the seam that would let a caller do it. When a person is
//! one day able to accept a certificate anyway, that will be a deliberate,
//! recorded act through queue item 127's security surfaces.

use crate::certificate::{Fault, Refused};
use std::io::{Read, Write};
use std::sync::Arc;

/// What this machine will trust to have signed a certificate.
#[derive(Clone)]
pub struct Trust {
    config: Arc<rustls::ClientConfig>,
    /// How many authorities went in, kept because the built configuration
    /// does not offer the count back and a caller wants to know it trusts
    /// something.
    anchors: usize,
}

impl core::fmt::Debug for Trust {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("Trust")
    }
}

impl Trust {
    /// What the operating system trusts.
    ///
    /// The platform's own store rather than a bundle compiled into us, and
    /// deliberately: an organisation that runs its own certificate authority
    /// has already told the operating system about it, and a browser that
    /// ignored that would be a browser nobody in an organisation could use. A
    /// bundle we shipped would also go stale the day after we shipped it.
    ///
    /// # Errors
    ///
    /// A sentence, when the store cannot be read at all.
    pub fn from_this_machine() -> Result<Self, String> {
        let found = rustls_native_certs::load_native_certs();
        if found.certs.is_empty() {
            let why: Vec<String> = found.errors.iter().map(ToString::to_string).collect();
            return Err(format!(
                "this machine has no certificates to trust: {}",
                why.join("; ")
            ));
        }
        let mut anchors = rustls::RootCertStore::empty();
        let (_, _) = anchors.add_parsable_certificates(found.certs);
        Ok(Self::with(anchors))
    }

    /// Trust exactly these, and nothing else.
    ///
    /// For a test, and for the day somebody pins an authority on purpose.
    /// **Not** a way to trust everything: an empty list trusts nothing, which
    /// is what it should mean.
    ///
    /// # Errors
    ///
    /// A sentence, when one of them is not a certificate.
    pub fn of(certificates: &[Vec<u8>]) -> Result<Self, String> {
        let mut anchors = rustls::RootCertStore::empty();
        for bytes in certificates {
            let held = rustls_pki_types::CertificateDer::from(bytes.clone());
            anchors
                .add(held)
                .map_err(|why| format!("not a certificate: {why}"))?;
        }
        Ok(Self::with(anchors))
    }

    fn with(anchors: rustls::RootCertStore) -> Self {
        let count = anchors.len();
        let mut config = rustls::ClientConfig::builder_with_provider(Arc::new(
            rustls::crypto::ring::default_provider(),
        ))
        .with_safe_default_protocol_versions()
        .unwrap_or_else(|_| unreachable_default())
        .with_root_certificates(anchors)
        .with_no_client_auth();
        // Which protocols this end speaks, best first — the server picks one
        // during the handshake, so the answer is known **before the first byte
        // of a request goes out**. That is the whole reason ALPN exists rather
        // than a version header: discovering the protocol afterwards would mean
        // sending a request and then sending it again, and a `POST` sent twice
        // is a payment made twice.
        config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];
        Self {
            config: Arc::new(config),
            anchors: count,
        }
    }

    /// How many authorities this trusts.
    pub fn len(&self) -> usize {
        self.anchors
    }

    /// Whether it trusts nobody, which trusts nothing.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// The provider's own defaults cannot fail to build; this is the shape the
/// type system asks for rather than a case that happens.
fn unreachable_default() -> rustls::ConfigBuilder<rustls::ClientConfig, rustls::WantsVerifier> {
    rustls::ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
        .with_protocol_versions(&[&rustls::version::TLS13])
        .unwrap_or_else(|_| unreachable_default())
}

/// Why a connection did not happen.
#[derive(Debug)]
pub enum TlsError {
    /// The certificate was refused, and this says what to tell somebody.
    Certificate(Box<Refused>),
    /// The host is not a name a certificate could be checked against.
    NotAHost {
        /// What was asked for.
        host: String,
    },
    /// Something below TLS went wrong — the stream ended, mostly.
    Stream {
        /// What happened.
        why: String,
    },
    /// The handshake failed for a reason that is not about the certificate.
    Handshake {
        /// What `rustls` said.
        why: String,
    },
}

impl core::fmt::Display for TlsError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            TlsError::Certificate(refused) => write!(f, "{refused}"),
            TlsError::NotAHost { host } => {
                write!(
                    f,
                    "{host:?} is not a host a certificate can be checked against"
                )
            }
            TlsError::Stream { why } => write!(f, "the connection ended: {why}"),
            TlsError::Handshake { why } => write!(f, "the secure handshake failed: {why}"),
        }
    }
}

impl std::error::Error for TlsError {}

/// A stream with TLS over it.
pub struct Secured<S: Read + Write> {
    connection: rustls::ClientConnection,
    stream: S,
}

impl<S: Read + Write> Secured<S> {
    /// Whether the other end proved who it was.
    ///
    /// Always true of a value that exists: [`secure`] does not hand one back
    /// otherwise. It is here so that a caller reading the code can see that it
    /// is true rather than take it on trust.
    pub fn is_verified(&self) -> bool {
        self.connection
            .peer_certificates()
            .is_some_and(|held| !held.is_empty())
    }

    /// Which protocol the two ends agreed on during the handshake.
    ///
    /// [`None`] when the server said nothing about ALPN, which is what an older
    /// server does and which means HTTP/1.1 — the protocol everybody speaks
    /// without having to say so.
    pub fn agreed_protocol(&self) -> Option<Vec<u8>> {
        self.connection.alpn_protocol().map(<[u8]>::to_vec)
    }
}

impl<S: Read + Write> core::fmt::Debug for Secured<S> {
    /// Deliberately says nothing about the stream or the session keys.
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("Secured")
    }
}

impl<S: Read + Write> Read for Secured<S> {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        rustls::Stream::new(&mut self.connection, &mut self.stream).read(buffer)
    }
}

impl<S: Read + Write> Write for Secured<S> {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        rustls::Stream::new(&mut self.connection, &mut self.stream).write(buffer)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        rustls::Stream::new(&mut self.connection, &mut self.stream).flush()
    }
}

/// Put TLS over a stream, and refuse if the other end cannot prove who it is.
///
/// # Errors
///
/// [`TlsError`], and when it is [`TlsError::Certificate`] the refusal inside it
/// carries what to tell a person — see [`crate::certificate`].
pub fn secure<S: Read + Write>(
    trust: &Trust,
    host: &str,
    mut stream: S,
) -> Result<Secured<S>, TlsError> {
    let name = rustls_pki_types::ServerName::try_from(host.to_owned()).map_err(|_| {
        TlsError::NotAHost {
            host: host.to_owned(),
        }
    })?;
    let mut connection = rustls::ClientConnection::new(Arc::clone(&trust.config), name)
        .map_err(|why| translate(host, &why))?;

    // Drive the handshake to the end before handing anything back, so that a
    // caller cannot write a password into a stream whose other end has not
    // proved who it is yet.
    while connection.is_handshaking() {
        if connection.wants_write() {
            connection
                .write_tls(&mut stream)
                .map_err(|why| TlsError::Stream {
                    why: why.to_string(),
                })?;
            stream.flush().map_err(|why| TlsError::Stream {
                why: why.to_string(),
            })?;
            continue;
        }
        if connection.wants_read() {
            let read = connection
                .read_tls(&mut stream)
                .map_err(|why| TlsError::Stream {
                    why: why.to_string(),
                })?;
            if read == 0 {
                return Err(TlsError::Stream {
                    why: "the other end stopped before the handshake finished".to_owned(),
                });
            }
            connection
                .process_new_packets()
                .map_err(|why| translate(host, &why))?;
            continue;
        }
        break;
    }
    Ok(Secured { connection, stream })
}

/// A `rustls` error as something a person can be told.
///
/// The mapping is the point of this file. Everything `rustls` knows about *why*
/// a certificate failed is here, and anything it grows later arrives as
/// [`Fault::Other`] rather than being mislabelled as one of the others.
fn translate(host: &str, why: &rustls::Error) -> TlsError {
    use rustls::CertificateError as Cert;
    let fault = match why {
        rustls::Error::InvalidCertificate(Cert::Expired | Cert::ExpiredContext { .. }) => {
            Fault::Expired
        }
        rustls::Error::InvalidCertificate(Cert::NotValidYet | Cert::NotValidYetContext { .. }) => {
            Fault::NotYetValid
        }
        rustls::Error::InvalidCertificate(
            Cert::NotValidForName | Cert::NotValidForNameContext { .. },
        ) => Fault::WrongHost {
            asked_for: host.to_owned(),
        },
        rustls::Error::InvalidCertificate(Cert::UnknownIssuer) => Fault::UnknownIssuer,
        rustls::Error::InvalidCertificate(Cert::BadSignature | Cert::BadEncoding) => {
            Fault::BadSignature
        }
        rustls::Error::InvalidCertificate(other) => Fault::Other {
            detail: format!("{other:?}"),
        },
        other => {
            return TlsError::Handshake {
                why: other.to_string(),
            };
        }
    };
    TlsError::Certificate(Box::new(Refused {
        host: host.to_owned(),
        fault,
    }))
}

#[cfg(test)]
mod tests {
    //! TLS, end to end, over a socket that never leaves this machine.
    //!
    //! These live here rather than in `tests/` for the reason the file's own
    //! documentation gives: `rustls` may be named in this file and nowhere
    //! else, and a test that starts a TLS *server* needs it. The gate checks
    //! that, and it caught this exact thing when the tests were written
    //! outside — which is the boundary working rather than getting in the way.
    //!
    //! **Nothing here touches the network.** A certificate authority is made at
    //! test time, a server is started on `127.0.0.1` with an ephemeral port,
    //! and the client is told to trust exactly that authority and nothing else.
    //! It is a real handshake with real certificate validation — the same code
    //! path a real site takes — and it works on an aeroplane.

    use super::{TlsError, Trust, secure};
    use crate::certificate::Fault;
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::sync::Arc;

    /// An authority, and a certificate it signed for a host.
    struct Issued {
        authority: Vec<u8>,
        certificate: Vec<u8>,
        key: Vec<u8>,
    }

    /// Make a certificate authority and have it sign for `host`.
    ///
    /// `valid` says whether the certificate is one that is in date; an expired one
    /// is how the refusal path is reached without waiting a year.
    fn issue(host: &str, in_date: bool) -> Issued {
        issue_from("alo test authority", host, in_date)
    }

    /// The same, from a named authority — so that "a different authority" is
    /// genuinely a different one rather than one that happens to share a name.
    fn issue_from(authority_name: &str, host: &str, in_date: bool) -> Issued {
        use rcgen::{CertificateParams, DistinguishedName, KeyPair};

        let mut authority_params = CertificateParams::new(Vec::new()).expect("parameters");
        authority_params.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
        authority_params.distinguished_name = DistinguishedName::new();
        authority_params
            .distinguished_name
            .push(rcgen::DnType::CommonName, authority_name);
        let authority_key = KeyPair::generate().expect("a key");
        let authority = authority_params
            .self_signed(&authority_key)
            .expect("an authority");

        let mut leaf = CertificateParams::new(vec![host.to_owned()]).expect("parameters");
        if in_date {
            leaf.not_before = rcgen::date_time_ymd(2020, 1, 1);
            leaf.not_after = rcgen::date_time_ymd(3000, 1, 1);
        } else {
            // Valid once, and not now.
            leaf.not_before = rcgen::date_time_ymd(2020, 1, 1);
            leaf.not_after = rcgen::date_time_ymd(2021, 1, 1);
        }
        let leaf_key = KeyPair::generate().expect("a key");
        let certificate = leaf
            .signed_by(&leaf_key, &authority, &authority_key)
            .expect("a certificate");

        Issued {
            authority: authority.der().to_vec(),
            certificate: certificate.der().to_vec(),
            key: leaf_key.serialize_der(),
        }
    }

    /// Start a TLS server on loopback that says one thing and stops.
    ///
    /// Returns the port it is listening on. The thread ends with the connection.
    fn serve(issued: &Issued, says: &'static str) -> u16 {
        serve_offering(issued, says, &[])
    }

    /// The same, offering these protocols by ALPN.
    fn serve_offering(issued: &Issued, says: &'static str, offers: &[&str]) -> u16 {
        let offered: Vec<Vec<u8>> = offers.iter().map(|name| name.as_bytes().to_vec()).collect();
        let certificate = vec![rustls_pki_types::CertificateDer::from(
            issued.certificate.clone(),
        )];
        let key = rustls_pki_types::PrivateKeyDer::try_from(issued.key.clone()).expect("a key");
        let config = rustls::ServerConfig::builder_with_provider(Arc::new(
            rustls::crypto::ring::default_provider(),
        ))
        .with_safe_default_protocol_versions()
        .expect("defaults")
        .with_no_client_auth()
        .with_single_cert(certificate, key)
        .expect("a server");
        let mut config = config;
        config.alpn_protocols = offered;

        let listener = TcpListener::bind("127.0.0.1:0").expect("a port");
        let port = listener.local_addr().expect("an address").port();
        std::thread::spawn(move || {
            let Ok((socket, _)) = listener.accept() else {
                return;
            };
            let Ok(mut connection) = rustls::ServerConnection::new(Arc::new(config)) else {
                return;
            };
            let mut socket = socket;
            let mut stream = rustls::Stream::new(&mut connection, &mut socket);
            // A failed handshake is the point of half these tests, so neither
            // write nor flush is worth reporting.
            let _ = stream.write_all(says.as_bytes());
            let _ = stream.flush();
        });
        port
    }

    fn connect(port: u16) -> TcpStream {
        TcpStream::connect(("127.0.0.1", port)).expect("the loopback server")
    }

    /// The whole point of ALPN, and the reason it is in the handshake rather
    /// than in a header: the answer is known **before the first byte of a
    /// request goes out**. A client that discovered the protocol afterwards
    /// would have to send the request again, and a `POST` sent twice is a
    /// payment made twice.
    #[test]
    fn a_server_offering_http_2_is_agreed_with_before_anything_is_sent() {
        let issued = issue("alo.test", true);
        let port = serve_offering(&issued, "", &["h2", "http/1.1"]);
        let trust = Trust::of(std::slice::from_ref(&issued.authority)).expect("an authority");
        let secured = secure(&trust, "alo.test", connect(port)).expect("a handshake");
        assert_eq!(
            secured.agreed_protocol().as_deref(),
            Some(&b"h2"[..]),
            "the server offered h2 first and it was not taken"
        );
    }

    /// A server that offers only HTTP/1.1 gets HTTP/1.1, without a request
    /// being sent to find out.
    #[test]
    fn a_server_offering_only_http_1_1_is_agreed_with_as_such() {
        let issued = issue("alo.test", true);
        let port = serve_offering(&issued, "", &["http/1.1"]);
        let trust = Trust::of(std::slice::from_ref(&issued.authority)).expect("an authority");
        let secured = secure(&trust, "alo.test", connect(port)).expect("a handshake");
        assert_eq!(secured.agreed_protocol().as_deref(), Some(&b"http/1.1"[..]));
    }

    /// An older server says nothing about ALPN at all, and that means HTTP/1.1
    /// — the protocol everybody speaks without having to say so.
    #[test]
    fn a_server_that_says_nothing_about_alpn_means_http_1_1() {
        let issued = issue("alo.test", true);
        let port = serve_offering(&issued, "", &[]);
        let trust = Trust::of(std::slice::from_ref(&issued.authority)).expect("an authority");
        let secured = secure(&trust, "alo.test", connect(port)).expect("a handshake");
        assert_eq!(
            secured.agreed_protocol(),
            None,
            "a server that said nothing was read as having said something"
        );
    }

    #[test]
    fn a_certificate_this_machine_trusts_connects_and_carries_bytes() {
        let issued = issue("alo.test", true);
        let port = serve(&issued, "hello from a secure connection");
        let trust = Trust::of(std::slice::from_ref(&issued.authority)).expect("an authority");
        assert_eq!(trust.len(), 1);
        assert!(!trust.is_empty());

        let mut secured = secure(&trust, "alo.test", connect(port)).expect("a handshake");
        let mut said = String::new();
        let _ = secured.read_to_string(&mut said);
        assert_eq!(said, "hello from a secure connection");
    }

    #[test]
    fn a_certificate_nobody_signed_for_is_refused_with_a_reason_in_words() {
        // The client is told to trust a *different* authority, which is what an
        // ordinary self-signed certificate looks like from outside.
        let issued = issue("alo.test", true);
        let somebody_else = issue_from("somebody else entirely", "alo.test", true);
        let port = serve(&issued, "should never be read");
        let trust =
            Trust::of(std::slice::from_ref(&somebody_else.authority)).expect("an authority");

        match secure(&trust, "alo.test", connect(port)) {
            Err(TlsError::Certificate(refused)) => {
                assert_eq!(refused.fault, Fault::UnknownIssuer);
                assert!(refused.what_is_wrong().contains("signed"), "{refused}");
                assert!(
                    refused.what_trusting_it_means().contains("trusting"),
                    "and what going on would mean: {refused}",
                );
                assert!(
                    refused.could_ever_be_trusted(),
                    "an organisation's own authority is the innocent explanation",
                );
            }
            other => panic!("expected a refusal, got {other:?}"),
        }
    }

    #[test]
    fn a_certificate_for_another_site_is_refused_and_is_never_something_to_go_on_from() {
        let issued = issue("somewhere-else.test", true);
        let port = serve(&issued, "should never be read");
        let trust = Trust::of(std::slice::from_ref(&issued.authority)).expect("an authority");

        match secure(&trust, "alo.test", connect(port)) {
            Err(TlsError::Certificate(refused)) => {
                assert_eq!(
                    refused.fault,
                    Fault::WrongHost {
                        asked_for: "alo.test".to_owned(),
                    },
                );
                assert!(refused.what_is_wrong().contains("alo.test"), "{refused}");
                assert!(
                    !refused.could_ever_be_trusted(),
                    "this is what an interception looks like",
                );
            }
            other => panic!("expected a refusal, got {other:?}"),
        }
    }

    #[test]
    fn a_certificate_that_has_run_out_is_refused_and_says_which_problem_it_is() {
        let issued = issue("alo.test", false);
        let port = serve(&issued, "should never be read");
        let trust = Trust::of(std::slice::from_ref(&issued.authority)).expect("an authority");

        match secure(&trust, "alo.test", connect(port)) {
            Err(TlsError::Certificate(refused)) => {
                assert_eq!(refused.fault, Fault::Expired);
                assert!(refused.what_is_wrong().contains("expired"), "{refused}");
                assert!(refused.could_ever_be_trusted(), "somebody forgot to renew");
            }
            other => panic!("expected a refusal, got {other:?}"),
        }
    }

    #[test]
    fn trusting_nobody_trusts_nothing() {
        // The closest thing to a bypass that this API can express, and it goes the
        // safe way: an empty list of authorities refuses everything rather than
        // accepting everything.
        let issued = issue("alo.test", true);
        let port = serve(&issued, "should never be read");
        let trust = Trust::of(&[]).expect("no authorities");
        assert!(trust.is_empty());
        assert!(
            secure(&trust, "alo.test", connect(port)).is_err(),
            "an empty trust store is not an open door",
        );
    }

    #[test]
    fn a_host_that_is_not_a_host_is_refused_before_anything_is_sent() {
        let trust = Trust::of(&[]).expect("no authorities");
        // No server is started: this must fail before it would need one.
        match secure(&trust, "not a host name", std::io::Cursor::new(Vec::new())) {
            Err(TlsError::NotAHost { host }) => assert_eq!(host, "not a host name"),
            other => panic!("expected a refusal, got {other:?}"),
        }
    }

    #[test]
    fn this_machine_trusts_somebody() {
        // Not a claim about which authorities — that is the operating system's
        // business, and the point of asking it rather than shipping a bundle that
        // goes stale. Only that reading the store works here.
        match Trust::from_this_machine() {
            Ok(trust) => assert!(!trust.is_empty(), "a machine with no roots trusts nothing"),
            Err(why) => panic!("could not read this machine's certificates: {why}"),
        }
    }
}
