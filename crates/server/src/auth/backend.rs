use super::{AuthError, AuthState};
use axum_login::{AuthUser, AuthnBackend};
use serde::Serialize;
use waxdemon_discogs::oauth::OAuthCredentials;

#[derive(Clone, Debug, Serialize, sqlx::FromRow)]
pub struct User {
    pub id: i64,
    pub discogs_id: i64,
    pub username: String,
    pub role: String,
    pub status: String,
    #[serde(skip)]
    session_revocation: String,
}

#[derive(Debug)]
pub struct LoginCredentials {
    pub access: OAuthCredentials,
    pub expected_user: Option<i64>,
}

impl AuthUser for User {
    type Id = i64;
    fn id(&self) -> i64 {
        self.id
    }
    fn session_auth_hash(&self) -> &[u8] {
        self.session_revocation.as_bytes()
    }
}

impl AuthnBackend for AuthState {
    type User = User;
    type Credentials = LoginCredentials;
    type Error = AuthError;

    async fn authenticate(&self, credentials: LoginCredentials) -> Result<Option<User>, AuthError> {
        let access = credentials.access;
        let identity = self
            .oauth
            .identity(&access)
            .await
            .map_err(|_| AuthError::Provider)?;
        let mut tx = self.pool.begin().await?;
        let user: User = if let Some(expected) = credentials.expected_user {
            let user=sqlx::query_as("UPDATE users SET username=$3 WHERE id=$1 AND discogs_id=$2 RETURNING id,discogs_id,username,role,status,session_revocation::text")
                .bind(expected).bind(identity.id).bind(identity.username).fetch_optional(&mut *tx).await?;
            let Some(user) = user else {
                return Ok(None);
            };
            user
        } else {
            sqlx::query_as(
                "INSERT INTO users (discogs_id, username) VALUES ($1, $2)
             ON CONFLICT (discogs_id) DO UPDATE SET username = EXCLUDED.username
             RETURNING id, discogs_id, username, role, status, session_revocation::text",
            )
            .bind(identity.id)
            .bind(identity.username)
            .fetch_one(&mut *tx)
            .await?
        };
        if !matches!(user.status.as_str(), "pending" | "approved") {
            tx.rollback().await?;
            return Ok(None);
        }
        let had_connection: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM discogs_connections WHERE user_id=$1)")
                .bind(user.id)
                .fetch_one(&mut *tx)
                .await?;
        let cancelled=sqlx::query("UPDATE user_sync_runs SET status='cancelled',phase='cancelled',error='Discogs credentials replaced',finished_at=now() WHERE user_id=$1 AND status IN ('queued','running')")
            .bind(user.id).execute(&mut *tx).await?.rows_affected();
        let encrypted = self
            .vault
            .encrypt(user.id, &access)
            .map_err(|_| AuthError::Internal)?;
        sqlx::query("INSERT INTO discogs_connections (user_id, key_id, nonce, ciphertext) VALUES ($1, $2, $3, $4)
            ON CONFLICT (user_id) DO UPDATE SET key_id = EXCLUDED.key_id, nonce = EXCLUDED.nonce, ciphertext = EXCLUDED.ciphertext, updated_at = now()")
            .bind(user.id).bind(encrypted.key_id).bind(encrypted.nonce).bind(encrypted.ciphertext)
            .execute(&mut *tx).await?;
        if !had_connection || cancelled > 0 || credentials.expected_user.is_some() {
            waxdemon_db::user_sync::enqueue(&mut tx, user.id).await?;
        }
        tx.commit().await?;
        Ok(Some(user))
    }

    async fn get_user(&self, id: &i64) -> Result<Option<User>, AuthError> {
        Ok(sqlx::query_as(
            "SELECT id, discogs_id, username, role, status, session_revocation::text FROM users
            WHERE id = $1 AND status IN ('pending', 'approved')",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?)
    }
}
