use std::time::Duration;

use reqwest::Client as HttpClient;
use serde::{de::DeserializeOwned, Serialize};

use crate::error::Error;

pub(crate) fn build_http_client() -> Result<HttpClient, Error> {
    Ok(HttpClient::builder()
        .timeout(Duration::from_secs(20))
        .build()?)
}

pub(crate) async fn get_json<T: DeserializeOwned>(
    http: &HttpClient,
    url: &str,
) -> Result<T, Error> {
    let resp = http.get(url).send().await?;
    parse_body(resp).await
}

pub(crate) async fn post_json<B: Serialize, T: DeserializeOwned>(
    http: &HttpClient,
    url: &str,
    body: &B,
) -> Result<T, Error> {
    let resp = http.post(url).json(body).send().await?;
    parse_body(resp).await
}

async fn parse_body<T: DeserializeOwned>(resp: reqwest::Response) -> Result<T, Error> {
    let status = resp.status();
    let text = resp.text().await?;
    if !status.is_success() {
        return Err(Error::from_response(status, &text));
    }
    Ok(serde_json::from_str(&text)?)
}
