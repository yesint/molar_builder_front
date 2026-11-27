use gloo_net::http::{Request, Response};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;
pub fn url(path: &str) -> String {
    format!(
        "{}{path}",
        option_env!("BUILDER_API_ORIGIN")
            .unwrap_or("")
            .trim_end_matches('/')
    )
}
async fn check(r: Result<Response, gloo_net::Error>) -> Result<Response, String> {
    let r = r.map_err(|e| e.to_string())?;
    if !r.ok() {
        let status = r.status();
        let msg = r.json::<Value>().await.unwrap_or_default();
        return Err(msg["error"]
            .as_str()
            .map(String::from)
            .unwrap_or_else(|| format!("Server returned HTTP {status}")));
    }
    Ok(r)
}
pub async fn get<T: DeserializeOwned>(path: &str) -> Result<T, String> {
    check(Request::get(&url(path)).send().await)
        .await?
        .json()
        .await
        .map_err(|e| e.to_string())
}
pub async fn post<T: DeserializeOwned>(
    path: &str,
    body: &impl Serialize,
    key: Option<&str>,
) -> Result<T, String> {
    let mut r = Request::post(&url(path));
    if let Some(k) = key {
        r = r.header("X-Builder-Job", k);
    }
    check(r.json(body).map_err(|e| e.to_string())?.send().await)
        .await?
        .json()
        .await
        .map_err(|e| e.to_string())
}
pub async fn upload(f: &web_sys::File) -> Result<Vec<crate::model::Upload>, String> {
    let name = f.name();
    let ext = name.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    let label: String = name
        .chars()
        .filter(|c| c.is_ascii_graphic() || *c == ' ')
        .collect();
    check(
        Request::post(&url(&format!("/api/upload/{ext}")))
            .header("X-Filename", &label)
            .body(f.clone())
            .map_err(|e| e.to_string())?
            .send()
            .await,
    )
    .await?
    .json()
    .await
    .map_err(|e| e.to_string())
}
pub async fn bytes(path: &str, key: Option<&str>) -> Result<Vec<u8>, String> {
    let mut r = Request::get(&url(path));
    if let Some(k) = key {
        r = r.header("X-Builder-Job", k);
    }
    check(r.send().await)
        .await?
        .binary()
        .await
        .map_err(|e| e.to_string())
}
