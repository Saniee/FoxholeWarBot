//! The one HTTP client every outbound call goes through.

use std::sync::OnceLock;
use std::time::Duration;

/// Ceiling on a whole request, headers and body.
///
/// The number that matters is not this one but the absence of it: a client from
/// `reqwest::Client::new()` has **no request timeout at all**. An API that
/// accepts the connection and then goes quiet leaves the await pending forever,
/// and inside a slash command that means a deferred interaction spinning until
/// Discord gives up on it — the user sees a command that never answers, and the
/// log says nothing, because nothing failed.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);

/// Separate and shorter: refusing to connect is a different failure from
/// connecting and stalling, and it should be reported faster.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

/// A shared client with those timeouts applied.
///
/// Shared rather than built per call because a `Client` owns a connection pool:
/// building one at each call site throws away keep-alive and pays for a fresh
/// TCP and TLS handshake every time, which the full map does 53 times.
///
/// Cloning a `Client` is cheap and shares the pool, so callers that need an
/// owned one can `.clone()` this without losing the benefit.
pub fn client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .connect_timeout(CONNECT_TIMEOUT)
            .build()
            // Only fails if the TLS backend won't initialize, which is an
            // environment that cannot serve a single request anyway. Better
            // loud than silently falling back to an untimed client.
            .expect("could not build the HTTP client")
    })
}
