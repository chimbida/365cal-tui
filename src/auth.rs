use keyring::Entry;
use oauth2::basic::BasicClient;
use oauth2::reqwest::async_http_client;
use oauth2::{
    AuthUrl, AuthorizationCode, ClientId, CsrfToken, PkceCodeChallenge, RedirectUrl,
    RefreshToken, TokenResponse, TokenUrl, Scope
};
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use url::Url;
use log::{info, warn};

// Keyring service name updated
const KEYRING_SERVICE: &str = "365cal-tui";
const KEYRING_USERNAME: &str = "microsoft_refresh_token";

fn save_refresh_token(refresh_token: &str) -> Result<(), keyring::Error> {
    let entry = Entry::new(KEYRING_SERVICE, KEYRING_USERNAME)?;
    entry.set_password(refresh_token)
}

fn load_refresh_token() -> Option<RefreshToken> {
    if let Ok(entry) = Entry::new(KEYRING_SERVICE, KEYRING_USERNAME) {
        if let Ok(token_secret) = entry.get_password() {
            return Some(RefreshToken::new(token_secret));
        }
    }
    None
}

fn delete_refresh_token() -> Result<(), keyring::Error> {
    let entry = Entry::new(KEYRING_SERVICE, KEYRING_USERNAME)?;
    entry.delete_password()
}

pub async fn authenticate(client_id_str: String) -> Result<String, Box<dyn std::error::Error>> {
    let client_id = ClientId::new(client_id_str);
    let client_secret = None;
    let auth_url = AuthUrl::new("https://login.microsoftonline.com/common/oauth2/v2.0/authorize".to_string())?;
    let token_url = Some(TokenUrl::new("https://login.microsoftonline.com/common/oauth2/v2.0/token".to_string())?);
    
    let redirect_url = RedirectUrl::new("http://localhost:8080".to_string())?;
    
    let client = BasicClient::new(client_id, client_secret, auth_url, token_url).set_redirect_uri(redirect_url);

    if let Some(saved_refresh_token) = load_refresh_token() {
        info!("Attempting to refresh access token from system keyring...");
        let token_result = client.exchange_refresh_token(&saved_refresh_token).request_async(async_http_client).await;
        if let Ok(refreshed_token) = token_result {
            info!("Token refreshed successfully!");
            if let Some(new_refresh_token) = refreshed_token.refresh_token() {
                save_refresh_token(new_refresh_token.secret())?;
            }
            return Ok(refreshed_token.access_token().secret().clone());
        } else {
            warn!("Could not refresh token. Deleting old token and starting full login...");
            let _ = delete_refresh_token();
        }
    }

    let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();
    let (authorize_url, _csrf_state) = client.authorize_url(CsrfToken::new_random)
        .add_scope(Scope::new("offline_access".to_string()))
        .add_scope(Scope::new("User.Read".to_string()))
        .add_scope(Scope::new("Calendars.Read".to_string()))
        .set_pkce_challenge(pkce_challenge).url();

    info!("Open this URL in your browser to log in: {}", authorize_url);
    println!("To continue, please open your browser and log in...");
    webbrowser::open(authorize_url.as_str())?;

    let listener = TcpListener::bind("127.0.0.1:8080")?;
    let mut code_option = None;
    if let Some(Ok(mut stream)) = listener.incoming().next() {
        let mut reader = BufReader::new(&stream);
        let mut request_line = String::new();
        reader.read_line(&mut request_line)?;
        if let Some(url_part) = request_line.split_whitespace().nth(1) {
            if url_part.contains("code=") {
                let full_url = Url::parse(&("http://localhost".to_string() + url_part))?;
                if let Some((_, value)) = full_url.query_pairs().find(|(key, _)| key == "code") {
                    code_option = Some(value.into_owned());
                }
            }
        }
        let message = "Login successful! You can now close this tab.";
        let response = format!("HTTP/1.1 200 OK\r\ncontent-length: {}\r\n\r\n{}", message.len(), message);
        stream.write_all(response.as_bytes())?;
    }

    if let Some(code) = code_option {
        let token_result = client.exchange_code(AuthorizationCode::new(code))
            .set_pkce_verifier(pkce_verifier).request_async(async_http_client).await;
        if let Ok(token) = token_result {
            if let Some(refresh_token) = token.refresh_token() {
                info!("Saving refresh token to system keyring...");
                save_refresh_token(refresh_token.secret())?;
            }
            return Ok(token.access_token().secret().clone());
        }
    }

    Err("Authentication failed".into())
}