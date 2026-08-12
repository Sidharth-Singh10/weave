//! Google OAuth/OIDC (Authorization Code flow).
//!
//! The provider is behind a small [`OidcProvider`] trait so the full Google
//! flow is exercised only against real Google, while unit/integration tests
//! use a mock. Never log authorization codes, access tokens, or id tokens.

use std::future::Future;
use std::pin::Pin;

use openidconnect::core::{CoreAuthenticationFlow, CoreClient, CoreProviderMetadata};
use openidconnect::{
    AuthorizationCode, ClientId, ClientSecret, CsrfToken, IssuerUrl, Nonce, RedirectUrl, Scope,
    TokenResponse,
};
// Re-exported reqwest (v0.12) — the one oauth2/openidconnect's HTTP trait is
// implemented for. Our own HTTP client (llm.rs) uses reqwest 0.13 and is
// unrelated.
use openidconnect::reqwest as oidc_reqwest;

/// Verified identity claims extracted from Google's id_token.
#[derive(Debug, Clone)]
pub struct OidcIdentity {
    pub subject: String,
    pub email: String,
    pub name: Option<String>,
    pub picture: Option<String>,
}

pub type ExchangeFuture<'a> =
    Pin<Box<dyn Future<Output = anyhow::Result<OidcIdentity>> + Send + 'a>>;

/// Abstraction over an OpenID Connect identity provider.
pub trait OidcProvider: Send + Sync {
    /// Build the authorization URL for a caller-supplied `state` and `nonce`.
    fn authorize_url(&self, state: &str, nonce: &str) -> String;

    /// Exchange the authorization `code` for an identity.
    fn exchange_code<'a>(&'a self, code: &'a str, nonce: &'a str) -> ExchangeFuture<'a>;
}

/// Google implementation backed by the `openidconnect` crate.
///
/// Stores the discovered provider metadata rather than a fully-built
/// `CoreClient`, whose endpoint-state type is not storable without a long
/// generic alias. Rebuilding the client from metadata is cheap (no network).
pub struct GoogleOidc {
    metadata: CoreProviderMetadata,
    client_id: String,
    client_secret: String,
    redirect_uri: String,
    http: oidc_reqwest::Client,
}

impl GoogleOidc {
    /// Discover Google's metadata and build a client. Fails when Google is
    /// unreachable or the client id/secret are invalid.
    pub async fn new(
        client_id: &str,
        client_secret: &str,
        redirect_uri: &str,
    ) -> anyhow::Result<Self> {
        // Following redirects opens the client up to SSRF vulnerabilities.
        let http = oidc_reqwest::Client::builder()
            .redirect(oidc_reqwest::redirect::Policy::none())
            .connect_timeout(std::time::Duration::from_secs(10))
            .build()?;

        let issuer = IssuerUrl::new("https://accounts.google.com".to_string())?;
        let metadata = CoreProviderMetadata::discover_async(issuer, &http).await?;
        Ok(Self {
            metadata,
            client_id: client_id.to_string(),
            client_secret: client_secret.to_string(),
            redirect_uri: redirect_uri.to_string(),
            http,
        })
    }
}

impl OidcProvider for GoogleOidc {
    fn authorize_url(&self, state: &str, nonce: &str) -> String {
        let csrf = state.to_string();
        let nonce = nonce.to_string();
        let client = CoreClient::from_provider_metadata(
            self.metadata.clone(),
            ClientId::new(self.client_id.clone()),
            Some(ClientSecret::new(self.client_secret.clone())),
        )
        .set_redirect_uri(
            RedirectUrl::new(self.redirect_uri.clone())
                .expect("redirect URI was validated at construction"),
        );
        let (auth_url, _csrf, _nonce) = client
            .authorize_url(
                CoreAuthenticationFlow::AuthorizationCode,
                move || CsrfToken::new(csrf),
                move || Nonce::new(nonce),
            )
            .add_scope(Scope::new("openid".to_string()))
            .add_scope(Scope::new("email".to_string()))
            .add_scope(Scope::new("profile".to_string()))
            .url();
        auth_url.to_string()
    }

    fn exchange_code<'a>(&'a self, code: &'a str, nonce: &'a str) -> ExchangeFuture<'a> {
        Box::pin(async move {
            let client = CoreClient::from_provider_metadata(
                self.metadata.clone(),
                ClientId::new(self.client_id.clone()),
                Some(ClientSecret::new(self.client_secret.clone())),
            )
            .set_redirect_uri(
                RedirectUrl::new(self.redirect_uri.clone())
                    .expect("redirect URI was validated at construction"),
            );
            let token_response = client
                .exchange_code(AuthorizationCode::new(code.to_string()))?
                .request_async(&self.http)
                .await
                .map_err(|e| anyhow::anyhow!("OAuth token exchange failed: {e:?}"))?;

            let id_token = token_response
                .id_token()
                .ok_or_else(|| anyhow::anyhow!("OAuth response contained no id_token"))?;

            let claims = id_token
                .claims(&client.id_token_verifier(), &Nonce::new(nonce.to_string()))
                .map_err(|e| anyhow::anyhow!("id_token verification failed: {e:?}"))?;

            let subject = claims.subject().to_string();
            let email = claims
                .email()
                .ok_or_else(|| anyhow::anyhow!("id_token is missing the email claim"))?;
            if !claims.email_verified().unwrap_or(false) {
                tracing::warn!(subject = %subject, "unverified email on login");
            }

            let name = claims
                .name()
                .and_then(|n| n.get(None))
                .map(|s| s.to_string());
            let picture = claims
                .picture()
                .and_then(|p| p.get(None))
                .map(|u| u.to_string());

            Ok(OidcIdentity {
                subject,
                email: email.to_string(),
                name,
                picture,
            })
        })
    }
}

/// Placeholder provider used when Google OAuth is not configured. Every use
/// returns a clear configuration error instead of a confusing network error.
pub struct UnconfiguredOidc;

impl OidcProvider for UnconfiguredOidc {
    fn authorize_url(&self, _state: &str, _nonce: &str) -> String {
        // Callers should check configuration before calling this; a generated
        // invalid URL is only reached if a route forgot the check.
        String::new()
    }

    fn exchange_code<'a>(&'a self, _code: &'a str, _nonce: &'a str) -> ExchangeFuture<'a> {
        Box::pin(async move { Err(anyhow::anyhow!("Google OAuth is not configured")) })
    }
}

/// A configurable mock identity provider for tests.
// Used by auth-flow integration tests (Phase 2 tests / Phase 8 e2e).
#[allow(dead_code)]
pub struct MockOidc {
    authorize_url: String,
    identity: OidcIdentity,
}

impl MockOidc {
    #[allow(dead_code)]
    pub fn new(authorize_url: impl Into<String>, identity: OidcIdentity) -> Self {
        Self {
            authorize_url: authorize_url.into(),
            identity,
        }
    }
}

impl OidcProvider for MockOidc {
    fn authorize_url(&self, state: &str, _nonce: &str) -> String {
        format!("{}?state={}", self.authorize_url, state)
    }

    fn exchange_code<'a>(&'a self, _code: &'a str, _nonce: &'a str) -> ExchangeFuture<'a> {
        let identity = self.identity.clone();
        Box::pin(async move { Ok(identity) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn mock_provider_round_trips_identity() {
        let mock = MockOidc::new(
            "https://idp.example/authorize",
            OidcIdentity {
                subject: "sub-123".to_string(),
                email: "a@example.com".to_string(),
                name: Some("Alice".to_string()),
                picture: None,
            },
        );
        let url = mock.authorize_url("state-1", "nonce-1");
        assert_eq!(url, "https://idp.example/authorize?state=state-1");

        let identity = mock.exchange_code("code-1", "nonce-1").await.unwrap();
        assert_eq!(identity.subject, "sub-123");
        assert_eq!(identity.email, "a@example.com");
        assert_eq!(identity.name.as_deref(), Some("Alice"));
    }

    #[tokio::test]
    async fn unconfigured_provider_errors() {
        let provider = UnconfiguredOidc;
        let err = provider.exchange_code("c", "n").await.unwrap_err();
        assert!(err.to_string().contains("not configured"));
    }
}
