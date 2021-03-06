use std::collections::HashMap;
use std::error::Error;
use std::str;

use portpicker::{pick_unused_port, Port};
use reqwest;
use reqwest::{RequestBuilder, Response};

use crate::auth::server;
use crate::auth::structs::RefreshResponse;
use crate::utils::config::{headers, SCOPES, SETTINGS};
use crate::utils::db;

pub async fn update_tokens(client: reqwest::Client) -> RefreshResponse {
    let token = db::read("config", "refresh_token".to_string());

    let tokens = {
        match token {
            Some(v) => {
                println!("Refreshing tokens");
                let res = refresh_tokens(client, v).await.unwrap();
                println!("Refreshed");
                res
            }
            None => {
                println!("Refresh token not found");
                run_oauth().await.unwrap()
            }
        }
    };
    db::write("config", "refresh_token".to_string(), tokens.refresh_token.to_string());

    return tokens;
}

pub async fn exchange_token(
    client: reqwest::Client, auth_code: &str, redirect_uri: String,
) -> Result<RefreshResponse, Box<dyn Error>> {
    let body = {
        let mut m = HashMap::new();
        m.insert("client_secret", SETTINGS.client_secret.as_str());
        m.insert("grant_type", "authorization_code");
        m.insert("code", auth_code);
        m.insert("redirect_uri", redirect_uri.as_str());
        m
    };

    let request = client
        .post("https://open-api.trovo.live/openplatform/exchangetoken")
        .headers(headers())
        .json(&body);

    let response = request.send().await?;

    return match response.status() {
        reqwest::StatusCode::OK => {
            let payload = response.json::<RefreshResponse>().await?;
            Ok(payload)
        }
        _ => Err(format!("Caught an invalid response: {:?}", response))?
    };
}

async fn refresh_tokens(
    client: reqwest::Client, token: String,
) -> Result<RefreshResponse, Box<dyn Error>> {
    let body: HashMap<&str, &str> = {
        let mut m: HashMap<&str, &str> = HashMap::new();
        m.insert("client_secret", SETTINGS.client_secret.as_str());
        m.insert("grant_type", "refresh_token");
        m.insert("refresh_token", token.as_str());
        m
    };

    let request: RequestBuilder = client
        .post("https://open-api.trovo.live/openplatform/refreshtoken")
        .headers(headers())
        .json(&body);

    let response: Response = request.send().await?;

    return match response.status() {
        reqwest::StatusCode::OK => {
            let payload = response.json::<RefreshResponse>().await?;
            Ok(payload)
        }
        _ => Err(format!("Caught an invalid response: {:?}", response))?
    };
}

pub async fn run_oauth() -> Result<RefreshResponse, Box<dyn Error>> {
    let port: Port = pick_unused_port().unwrap();

    // User must open this link and login to account of bot
    let redirect_uri: String = format!("http://localhost:{}", port);
    let auth_url: String = format!(
        "Go to link:\nhttps://open.trovo.live/page/login.html?client_id={}&response_type=code&scope={}&redirect_uri={}",
        SETTINGS.client_id, SCOPES.join("+"), redirect_uri
    );
    println!("{}", auth_url);

    // Out server is blocking the main thread and waiting for redirect from Trovo login page
    let code: String = server::oauth_server(port);
    // Get refresh and access token
    return exchange_token(reqwest::Client::new(), code.as_str(), redirect_uri).await;
}