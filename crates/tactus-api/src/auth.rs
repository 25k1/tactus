use crate::error::{Error, Result};
use crate::model::DeviceCode;
use crate::net;
use crate::parse::{self, Json};
use serde_json::Value;

const OAUTH_URL: &str = "https://oauth.yandex.ru";
pub const CLIENT_ID: &str = "23cabbbdc6cd418abb4b39c32c41195d";
const CLIENT_SECRET: &str = "53bc75238f0c4d08a118e51fe9203300";

async fn post(path: &str, form: &[(&str, &str)]) -> Result<Value> {
    let response = net::client().post(format!("{OAUTH_URL}/{path}")).form(form).send().await?;
    let success = response.status().is_success();
    let value: Value = response.json().await?;
    if success {
        return Ok(value);
    }
    Err(match value.text("error") {
        "authorization_pending" | "slow_down" => Error::AuthorizationPending,
        _ => Error::Api(match value.text("error_description") {
            "" => value.text("error").to_owned(),
            description => description.to_owned(),
        }),
    })
}

pub fn request_device_code() -> Result<DeviceCode> {
    let value = net::block_on(post("device/code", &[("device_name", "Tactus"), ("client_id", CLIENT_ID)]))?;
    Ok(parse::device_code(&value))
}

pub fn poll_device_token(device_code: &str) -> Result<String> {
    let value = net::block_on(post(
        "token",
        &[
            ("grant_type", "device_code"),
            ("code", device_code),
            ("client_id", CLIENT_ID),
            ("client_secret", CLIENT_SECRET),
        ],
    ))?;
    match value.text("access_token") {
        "" => Err(Error::Api("no access token in the response".into())),
        token => Ok(token.to_owned()),
    }
}
