use std::{collections::HashMap, path::Path};

use anyhow::Context;
use argon2::{password_hash::SaltString, Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use rand::rngs::OsRng;
use tokio::{
    fs::File,
    io::{AsyncReadExt, AsyncWriteExt},
    sync::RwLock,
};
use tracing::{debug, trace};

const USERS_SAVE_FILE: &str = "./users.json";

pub struct UserManager {
    /// Key: Username
    /// Value: Hash
    users: RwLock<HashMap<String, String>>,
}

impl UserManager {
    pub async fn new_from_save_file() -> anyhow::Result<Self> {
        let users = if Path::new(USERS_SAVE_FILE).exists() {
            let mut file = File::open(USERS_SAVE_FILE)
                .await
                .context(format!("Failed to read users save file {USERS_SAVE_FILE}"))?;

            // As the file is not so big, I'm fine with reading it into a buffer.
            // I want to avoid taking a dependency on e.g. https://github.com/carllerche/tokio-serde
            let mut contents = Vec::with_capacity(
                file.metadata()
                    .await
                    .context(format!(
                        "Failed to read metadata of users save file {USERS_SAVE_FILE}"
                    ))?
                    .len() as usize,
            );
            file.read_to_end(&mut contents).await.context(format!(
                "Failed to read from users save file {USERS_SAVE_FILE}"
            ))?;

            serde_json::from_slice(&contents).context("Failed to deserialize users save file")?
        } else {
            Default::default()
        };

        Ok(Self {
            users: RwLock::new(users),
        })
    }

    async fn write_users_to_file(&self) -> anyhow::Result<()> {
        let content = serde_json::to_vec(&*self.users.read().await)
            .context("Failed to serialize users save file")?;
        let num_bytes = content.len();

        let mut file = File::create(USERS_SAVE_FILE)
            .await
            .context(format!("Failed to open users save file {USERS_SAVE_FILE}"))?;
        file.write_all(&content).await.context(format!(
            "Failed to write to users save file {USERS_SAVE_FILE}"
        ))?;

        trace!(num_bytes, "Written users save file");

        Ok(())
    }

    /// Checks the given username and password.
    ///
    /// In case the username is not already taken, this is counted as user registration and a
    /// corresponding user is created.
    pub async fn check_credentials(&self, username: &str, password: &str) -> anyhow::Result<bool> {
        if let Some(password_hash) = self.users.read().await.get(username) {
            let password_hash = PasswordHash::new(password_hash).context(format!(
                "Failed to parse password hash for user {username}: {password_hash}"
            ))?;
            return Ok(Argon2::default()
                .verify_password(password.as_bytes(), &password_hash)
                .is_ok());
        }

        // password_hash is dropped here, so "create_user" can take the lock on "self.users"
        self.create_user(username, password)
            .await
            .context(format!("Failed to create user {username}"))?;

        // As we just created the user, the password is correct
        Ok(true)
    }

    async fn create_user(&self, username: &str, password: &str) -> anyhow::Result<()> {
        let argon2 = Argon2::default();
        let salt = SaltString::generate(&mut OsRng);
        let password_hash = argon2
            .hash_password(password.as_bytes(), &salt)
            .context("")?
            .to_string();

        debug!(username, password_hash, "Creating user");
        (*self.users.write().await).insert(username.to_owned(), password_hash);

        self.write_users_to_file()
            .await
            .context("Failed writing users to users save file")?;

        Ok(())
    }
}
