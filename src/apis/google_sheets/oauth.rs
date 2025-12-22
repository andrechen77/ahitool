use std::net::SocketAddr;
use std::path::Path;

use chrono::{DateTime, Utc};
use http::StatusCode;
use oauth2::basic::BasicTokenResponse;
use oauth2::{
    AuthUrl, ClientId, ClientSecret, CsrfToken, PkceCodeChallenge, Scope, TokenUrl,
    basic::BasicClient,
};
use oauth2::{AuthorizationCode, RedirectUrl, RefreshToken, TokenResponse};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::net::TcpListener;
use thiserror::Error;
use tiny_http::Response;
use tracing::{debug, trace, warn};
use url::Url;

use crate::utils;

pub type Token = BasicTokenResponse;

// const DEFAULT_CACHE_FILE: &str = "google_oauth_token.json";
const CLIENT_ID: &str = "859579651850-t212eiscr880fnifmsi6ddft2bhdtplt.apps.googleusercontent.com";
// It should be fine that the secret is not actually kept secret. see
// https://developers.google.com/identity/protocols/oauth2
const CLIENT_SECRET: &str = "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";
const AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const SCOPE_DRIVE_FILE: &str = "https://www.googleapis.com/auth/drive.file";

#[derive(Debug, Serialize, Deserialize)]
struct TokenWithExpiration {
    token: Token,
    time_obtained: DateTime<Utc>,
}

#[derive(Error, Debug)]
pub enum TryWithCredentialsError {
    #[error("the OAuth credentials are invalid")]
    Unauthorized(anyhow::Error),
    #[error("an error occurred while running an operation using OAuth credentials")]
    Other(#[from] anyhow::Error),
}

/// Runs a function that requires OAuth credentials. If the provided function
/// returns an error, this is interpreted as the credentials being invalid.
pub fn run_with_credentials<F, U>(cache_file: &Path, mut function: F) -> anyhow::Result<U>
where
    F: FnMut(&Token) -> Result<U, TryWithCredentialsError>,
{
    // attempt to run the function with a cached token
    let expired_token = match get_cached_token(cache_file) {
        Some((cached_token, false)) => {
            // attempt to run the function with the cached token
            trace!("Using cached token to perform operation");
            match function(&cached_token.token) {
                Ok(result) => {
                    // the function worked the first time. since we did not
                    // refresh anything, we do not need to cache the token again
                    return Ok(result);
                }
                Err(TryWithCredentialsError::Unauthorized(e)) => {
                    debug!("Cached token is invalid, as indicated by error: {}", e);
                    // even though `get_cached_token` returned `false`, the
                    // token might still be expired (either expired in between
                    // when we last checked till now, or it didn't have an
                    // indicated expiration date
                    Some(cached_token)
                }
                Err(TryWithCredentialsError::Other(e)) => {
                    // the problem was not with the credentials, so just return
                    // this error
                    return Err(e);
                }
            }
        }
        Some((cached_token, true)) => {
            // the token is known to be expired
            debug!("Cached token is expired");
            Some(cached_token)
        }
        None => None,
    };

    // attempt to refresh and run again
    'refresh: {
        let Some(expired_token) = expired_token else {
            debug!("No cached token to refresh");
            break 'refresh;
        };
        let Some(refresh_token) = expired_token.token.refresh_token() else {
            debug!("Cached token does not have a refresh token");
            break 'refresh;
        };
        trace!("Found refresh token. attempting to refresh");
        let refreshed_token = match refresh_credentials(refresh_token) {
            Ok(refreshed_token) => {
                debug!("Successfully refreshed token");
                refreshed_token
            }
            Err(e) => {
                warn!("Failed to refresh OAuth credentials: {}", e);
                break 'refresh;
            }
        };
        trace!("Performing operation with refreshed token");
        match function(&refreshed_token.token) {
            Ok(result) => {
                // the function worked with a refreshed token. cache this
                // refreshed token
                debug!("Caching refreshed token to {}", cache_file.display());
                let writer = BufWriter::new(File::create(cache_file)?);
                serde_json::to_writer(writer, &refreshed_token)?;
                return Ok(result);
            }
            Err(TryWithCredentialsError::Unauthorized(e)) => {
                debug!("Refreshed token is invalid, as indicated by error: {}", e);
            }
            Err(TryWithCredentialsError::Other(e)) => {
                // the problem was not with the credentials, so just return
                // this error
                return Err(e);
            }
        }
    }

    // getting to this point means the refreshed token did not work. attempt
    // to get totally fresh credentials and run again
    trace!("Attempting to get totally fresh credentials");
    let fresh_token = match get_fresh_credentials() {
        Ok(fresh_token) => fresh_token,
        Err(e) => {
            warn!("Failed to get fresh OAuth credentials: {}", e);
            return Err(e);
        }
    };
    let err = match function(&fresh_token.token) {
        Ok(result) => {
            // the function worked with a fresh token
            debug!("Caching fresh token to {}", cache_file.display());
            let writer = BufWriter::new(File::create(cache_file)?);
            serde_json::to_writer(writer, &fresh_token)?;
            return Ok(result);
        }
        Err(TryWithCredentialsError::Unauthorized(e)) => {
            warn!("The OAuth credentials are invalid even after getting a fresh token: {}", e);
            e
        }
        Err(TryWithCredentialsError::Other(e)) => {
            // the problem was not with the credentials, so just return
            // this error
            e
        }
    };
    Err(err)
}

// Returns the token from the cache file, as well as if the token is known to
// be expired.
fn get_cached_token(cache_file: &Path) -> Option<(TokenWithExpiration, bool)> {
    match cache_file.try_exists() {
        Ok(false) => {
            debug!("Cache file does not exist");
            return None;
        }
        Err(e) => {
            warn!("Unable to check if the cache file exists: {}", e);
            return None;
        }
        Ok(true) => {
            trace!("Found cache file");
        }
    }

    // at this point we know the file must exist
    let file = match File::open(cache_file) {
        Ok(file) => file,
        Err(e) => {
            warn!("Failed to open cache file: {}", e);
            // if we can't open the file even though `try_exists` returned
            // `Ok(true)`, it's probably because the file was deleted between
            // when we checked and when we we tried to open it, so we should
            // still attempt to cache the token
            return None;
        }
    };

    let cached_token: serde_json::Result<TokenWithExpiration> =
        serde_json::from_reader(BufReader::new(file));
    match cached_token {
        Ok(cached_token) => {
            debug!("Successfully deserialized cached token");
            if let Some(duration) = cached_token.token.expires_in() {
                let is_expired = cached_token.time_obtained + duration <= Utc::now();
                Some((cached_token, is_expired))
            } else {
                debug!("The token did not have an expiration time; assuming it is valid");
                Some((cached_token, false))
            }
        }
        Err(e) => {
            warn!("Failed to deserialize cached token: {}", e);
            None
        }
    }
}

fn refresh_credentials(refresh_token: &RefreshToken) -> anyhow::Result<TokenWithExpiration> {
    let time_obtained = Utc::now();
    let mut token =
        oauth2_client().exchange_refresh_token(refresh_token).request(oauth2::ureq::http_client)?;
    token.set_refresh_token(Some(refresh_token.clone()));
    Ok(TokenWithExpiration { token, time_obtained })
}

fn get_fresh_credentials() -> anyhow::Result<TokenWithExpiration> {
    // get the current time so we can calculate the expiration date
    let time_obtained = Utc::now();

    // establish a server to listen for the authorization code
    let addr: SocketAddr = ([127, 0, 0, 1], 0).into(); // request any port
    let tcp_listener = TcpListener::bind(addr)?;

    // create OAuth2 client
    let client = oauth2_client().set_redirect_uri(
        RedirectUrl::new(format!(
            "http://localhost:{}",
            tcp_listener.local_addr().expect("should exist").port(),
        ))
        .expect("hardcoded URL should be valid"),
    );
    let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();
    let (auth_url, csrf_token) = client
        .authorize_url(CsrfToken::new_random)
        .add_scope(Scope::new(SCOPE_DRIVE_FILE.to_string()))
        .set_pkce_challenge(pkce_challenge)
        .url();

    utils::open_url(auth_url.as_str());
    let code = listen_for_code(tcp_listener, csrf_token)?;

    let token = client
        .exchange_code(AuthorizationCode::new(code))
        .set_pkce_verifier(pkce_verifier)
        .request(oauth2::ureq::http_client)?;

    Ok(TokenWithExpiration { token, time_obtained })
}

fn listen_for_code(tcp_listener: TcpListener, csrf_token: CsrfToken) -> anyhow::Result<String> {
    // mapping the error is required because otherwise it won't compile for some
    // strange reason that I don't feel like diagnosing
    let server = tiny_http::Server::from_listener(tcp_listener, None)
        .map_err(|e| anyhow::Error::msg(e.to_string()))?;

    'request_loop: for req in server.incoming_requests() {
        // prepend a dummy protocol and host name so that it can be parsed
        let path_and_query = req.url();
        let url = format!("http://localhost{}", path_and_query);
        let Ok(url) = Url::parse(&url) else {
            warn!("Failed to parse path to requested resource: {}", path_and_query);
            continue 'request_loop;
        };

        if url.path() != "/" {
            let response = Response::empty(StatusCode::NO_CONTENT.as_u16());
            req.respond(response)?;
            continue 'request_loop;
        }

        // find the code and verify the state in the query string
        let code = {
            let mut code = None;
            let mut state_matches = false;
            for (k, v) in url.query_pairs() {
                match k.as_ref() {
                    "code" => code = Some(v),
                    "state" => {
                        if *csrf_token.secret() == v {
                            state_matches = true;
                        } else {
                            // ignore the rest of this request as it is invalid
                            break;
                        }
                    }
                    _ => (),
                }
            }
            if state_matches {
                if let Some(code) = code {
                    code
                } else {
                    if let Err(e) = req.respond(Response::from_string("Authorization code not found in redirect. Try again or contact the developer.")) {
                        warn!("Failed to respond to request: {}", e);
                    }
                    continue 'request_loop;
                }
            } else {
                // the request did not include a valid state, so it must be
                // rejected
                warn!(
                    "Authorization redirect did not include a valid state. This may be an indication of an attempted attack."
                );
                if let Err(e) = req.respond(Response::from_string("Authorization code rejected due to invalid state. Try again or contact the developer.")) {
                    warn!("Failed to respond to request: {}", e);
                }
                continue 'request_loop;
            }
        };

        req.respond(Response::from_string(
            "Authorization code received. You can now close this window.",
        ))?;
        return Ok(code.into_owned());
    }

    // getting here means the server stopped listening
    return Err(anyhow::anyhow!("server stopped listening for authorization code"));
}

fn oauth2_client() -> BasicClient {
    BasicClient::new(
        ClientId::new(CLIENT_ID.to_owned()),
        Some(ClientSecret::new(CLIENT_SECRET.to_owned())),
        AuthUrl::new(AUTH_URL.to_owned()).expect("hardcoded URL should be valid"),
        Some(TokenUrl::new(TOKEN_URL.to_owned()).expect("hardcoded URL should be valid")),
    )
}
