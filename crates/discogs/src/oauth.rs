use crate::client::{DISCOGS_API_BASE_URL, USER_AGENT};
use oauth1_request::{Builder, Credentials, PLAINTEXT};
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, HeaderValue};
use secrecy::{ExposeSecret, SecretSlice, SecretString};
use serde::Deserialize;
use std::collections::HashMap;
use std::time::Duration;
use url::Url;

const MAX_RESPONSE_BYTES: usize = 16 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum OAuthError {
    #[error("invalid OAuth configuration or credentials")]
    InvalidConfiguration,
    #[error("Discogs OAuth request failed")]
    Transport,
    #[error("Discogs OAuth returned HTTP {0}")]
    Http(u16),
    #[error("invalid Discogs OAuth response")]
    InvalidResponse,
}

#[derive(Debug, Clone)]
pub struct OAuthCredentials {
    token: SecretString,
    secret: SecretString,
}

impl OAuthCredentials {
    pub fn new(token: SecretString, secret: SecretString) -> Result<Self, OAuthError> {
        if token.expose_secret().trim().is_empty() || secret.expose_secret().trim().is_empty() {
            return Err(OAuthError::InvalidConfiguration);
        }
        Ok(Self { token, secret })
    }

    pub fn token(&self) -> &SecretString {
        &self.token
    }

    pub fn secret(&self) -> &SecretString {
        &self.secret
    }

    fn signing_credentials(&self) -> Credentials<&str> {
        Credentials::new(self.token.expose_secret(), self.secret.expose_secret())
    }
}

#[derive(Debug)]
pub struct RequestToken(OAuthCredentials);

impl RequestToken {
    pub fn from_credentials(credentials: OAuthCredentials) -> Self {
        Self(credentials)
    }

    pub fn credentials(&self) -> &OAuthCredentials {
        &self.0
    }

    pub fn authorization_url(&self) -> Url {
        let mut url = Url::parse("https://www.discogs.com/oauth/authorize").unwrap();
        url.query_pairs_mut()
            .append_pair("oauth_token", self.0.token.expose_secret());
        url
    }
}

#[derive(Debug, Deserialize)]
pub struct Identity {
    pub id: i64,
    pub username: String,
}

#[derive(Debug, Clone)]
pub struct OAuthClient {
    http: reqwest::Client,
    consumer: OAuthCredentials,
    base: Url,
}

impl OAuthClient {
    pub fn collection_client(&self, access: OAuthCredentials) -> Result<crate::Client, OAuthError> {
        crate::Client::with_oauth(self.consumer.clone(), access, self.base.as_str())
    }
    pub fn new(consumer: OAuthCredentials) -> Result<Self, OAuthError> {
        Self::with_base(consumer, DISCOGS_API_BASE_URL)
    }

    pub fn with_base(consumer: OAuthCredentials, base: &str) -> Result<Self, OAuthError> {
        let base = validate_url(base)?;
        if base.path() != "/" || base.query().is_some() {
            return Err(OAuthError::InvalidConfiguration);
        }
        let http = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .timeout(Duration::from_secs(20))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| OAuthError::InvalidConfiguration)?;
        Ok(Self {
            http,
            consumer,
            base,
        })
    }

    pub async fn request_token(&self, callback: &str) -> Result<RequestToken, OAuthError> {
        validate_url(callback)?;
        self.request_token_with_callback(callback).await
    }

    pub async fn request_token_out_of_band(&self) -> Result<RequestToken, OAuthError> {
        self.request_token_with_callback("oob").await
    }

    async fn request_token_with_callback(
        &self,
        callback: &str,
    ) -> Result<RequestToken, OAuthError> {
        let url = self.base.join("/oauth/request_token").unwrap();
        let header = Builder::<_, _, &str>::new(self.consumer.signing_credentials(), PLAINTEXT)
            .callback(callback)
            .get(url.as_str(), &());
        let body = self.send(reqwest::Method::GET, url, header).await?;
        let mut fields = token_fields(&body)?;
        if fields
            .remove("oauth_callback_confirmed")
            .as_ref()
            .map(ExposeSecret::expose_secret)
            != Some("true")
        {
            return Err(OAuthError::InvalidResponse);
        }
        Ok(RequestToken(credentials_from_fields(fields)?))
    }

    pub async fn exchange(
        &self,
        request: &RequestToken,
        verifier: &SecretString,
    ) -> Result<OAuthCredentials, OAuthError> {
        if verifier.expose_secret().trim().is_empty() {
            return Err(OAuthError::InvalidConfiguration);
        }
        let url = self.base.join("/oauth/access_token").unwrap();
        let header = Builder::new(self.consumer.signing_credentials(), PLAINTEXT)
            .token(request.0.signing_credentials())
            .verifier(verifier.expose_secret())
            .post(url.as_str(), &());
        let body = self.send(reqwest::Method::POST, url, header).await?;
        credentials_from_fields(token_fields(&body)?)
    }

    pub async fn identity(&self, access: &OAuthCredentials) -> Result<Identity, OAuthError> {
        let url = self.base.join("/oauth/identity").unwrap();
        let header = Builder::new(self.consumer.signing_credentials(), PLAINTEXT)
            .token(access.signing_credentials())
            .get(url.as_str(), &());
        let body = self.send(reqwest::Method::GET, url, header).await?;
        let identity: Identity = serde_json::from_slice(body.expose_secret())
            .map_err(|_| OAuthError::InvalidResponse)?;
        if identity.id <= 0 || identity.username.trim().is_empty() {
            return Err(OAuthError::InvalidResponse);
        }
        Ok(identity)
    }

    async fn send(
        &self,
        method: reqwest::Method,
        url: Url,
        authorization: String,
    ) -> Result<SecretSlice<u8>, OAuthError> {
        let authorization = SecretString::from(authorization);
        let mut header = HeaderValue::from_str(authorization.expose_secret())
            .map_err(|_| OAuthError::InvalidConfiguration)?;
        header.set_sensitive(true);
        let mut response = self
            .http
            .request(method, url)
            .header(AUTHORIZATION, header)
            .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
            .send()
            .await
            .map_err(|_| OAuthError::Transport)?;
        if response.status() != reqwest::StatusCode::OK {
            return Err(OAuthError::Http(response.status().as_u16()));
        }
        let mut body = secrecy::SecretBox::new(Box::new(Vec::new()));
        while let Some(chunk) = response.chunk().await.map_err(|_| OAuthError::Transport)? {
            use secrecy::ExposeSecretMut;
            if body.expose_secret().len().saturating_add(chunk.len()) > MAX_RESPONSE_BYTES {
                return Err(OAuthError::InvalidResponse);
            }
            body.expose_secret_mut().extend_from_slice(&chunk);
        }
        Ok(body.expose_secret().clone().into())
    }
}

pub(crate) fn validate_url(value: &str) -> Result<Url, OAuthError> {
    let url = Url::parse(value).map_err(|_| OAuthError::InvalidConfiguration)?;
    let local = match url.host() {
        Some(url::Host::Ipv4(ip)) => ip.is_loopback(),
        Some(url::Host::Ipv6(ip)) => ip.is_loopback(),
        _ => false,
    };
    if url.host().is_none()
        || !(url.scheme() == "https" || (url.scheme() == "http" && local))
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
    {
        return Err(OAuthError::InvalidConfiguration);
    }
    Ok(url)
}

fn token_fields(body: &SecretSlice<u8>) -> Result<HashMap<String, SecretString>, OAuthError> {
    std::str::from_utf8(body.expose_secret()).map_err(|_| OAuthError::InvalidResponse)?;
    let mut fields = HashMap::new();
    for (key, value) in url::form_urlencoded::parse(body.expose_secret()) {
        if fields
            .insert(key.into_owned(), SecretString::from(value.into_owned()))
            .is_some()
        {
            return Err(OAuthError::InvalidResponse);
        }
    }
    Ok(fields)
}

fn credentials_from_fields(
    mut fields: HashMap<String, SecretString>,
) -> Result<OAuthCredentials, OAuthError> {
    OAuthCredentials::new(
        fields
            .remove("oauth_token")
            .ok_or(OAuthError::InvalidResponse)?,
        fields
            .remove("oauth_token_secret")
            .ok_or(OAuthError::InvalidResponse)?,
    )
    .map_err(|_| OAuthError::InvalidResponse)
}
