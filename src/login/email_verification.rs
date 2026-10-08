//! Native Palpo email OTP; credentials never leave the selected homeserver.
use anyhow::{bail, Result};
use serde_json::{json, Value};
use url::Url;

#[derive(Clone)]
pub struct EmailProof {
    pub homeserver: String,
    pub sid: String,
    pub client_secret: String,
    pub registration_session: std::sync::Arc<std::sync::Mutex<Option<String>>>,
}
impl std::fmt::Debug for EmailProof {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EmailProof").field("homeserver", &self.homeserver).finish_non_exhaustive()
    }
}

async fn post(server: &str, route: &str, body: Value) -> Result<Value> {
    let url = Url::parse(server)?.join(route)?;
    // No redirects: OTP and client_secret belong only to the selected origin.
    let http = crate::http::client_builder()
        .redirect(matrix_sdk::reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(25)).build()?;
    let response = http.post(url).header("Content-Type", "application/json").body(body.to_string()).send().await?;
    let status = response.status();
    let result: Value = serde_json::from_slice(&response.bytes().await?)?;
    if !status.is_success() {
        bail!("{}", result["error"].as_str().unwrap_or("The server could not verify your email. Try again."));
    }
    Ok(result)
}

pub async fn send(server: &str, email: &str, secret: &str, attempt: u64) -> Result<String> {
    let result = post(server, "_matrix/client/v3/register/email/requestToken", json!({
        "email":email, "client_secret":secret, "send_attempt":attempt,
    })).await?;
    // Use only the Palpo endpoint advertised by the feature flag. Never post a
    // proof to an arbitrary submit_url supplied by the server.
    let sid = result["sid"].as_str().filter(|s| !s.is_empty() && s.len() <= 128)
        .ok_or_else(|| anyhow::anyhow!("The server did not return an email verification session."))?;
    Ok(sid.to_owned())
}

pub async fn verify(proof: &EmailProof, code: &str) -> Result<()> {
    let result = post(&proof.homeserver, "_matrix/client/v3/register/email/submitToken", json!({
        "sid":proof.sid,"client_secret":proof.client_secret,"token":code,
    })).await?;
    if result["success"] != true { bail!("The server could not verify your email. Try again.") }
    Ok(())
}
