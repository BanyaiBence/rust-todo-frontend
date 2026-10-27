//! Typed client for the todo-backend JSON API.

use std::fmt;

use reqwest::{RequestBuilder, Response, StatusCode};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

pub const DEFAULT_BASE_URL: &str = "http://localhost:3000";

/// Largest page the backend accepts for `GET /todos`.
pub const MAX_PAGE_SIZE: u32 = 100;

/// Base URL from `TODO_API_URL`, falling back to [`DEFAULT_BASE_URL`].
pub fn base_url_from_env() -> String {
    std::env::var("TODO_API_URL")
        .ok()
        .filter(|url| !url.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_BASE_URL.to_string())
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct User {
    pub id: String,
    pub username: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Token {
    pub token: String,
    pub token_type: String,
    pub expires_in: u64,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Todo {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub completed: bool,
    pub created_at: String,
    pub updated_at: String,
}

/// Body of `POST /todos`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct NewTodo {
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Body of `PATCH /todos/{id}`. `None` fields are omitted and left unchanged;
/// `description: Some(None)` sends `null`, which clears the description.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct TodoChanges {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<Option<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed: Option<bool>,
}

impl TodoChanges {
    pub fn is_empty(&self) -> bool {
        self.title.is_none() && self.description.is_none() && self.completed.is_none()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ApiError {
    /// `401`: missing, invalid or expired token, or wrong login credentials.
    Unauthorized,
    /// Any other non-success status, with the backend's `error` message.
    Status { status: u16, message: String },
    /// The request didn't complete or the response couldn't be read.
    Transport(String),
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ApiError::Unauthorized => write!(f, "unauthorized"),
            ApiError::Status { message, .. } => write!(f, "{message}"),
            ApiError::Transport(message) => write!(f, "could not reach the server: {message}"),
        }
    }
}

impl std::error::Error for ApiError {}

#[derive(Deserialize)]
struct ErrorBody {
    error: String,
}

#[derive(Serialize)]
struct Credentials<'a> {
    username: &'a str,
    password: &'a str,
}

#[derive(Debug, Clone)]
pub struct Client {
    http: reqwest::Client,
    base_url: String,
    token: Option<String>,
}

impl Client {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            http: reqwest::Client::new(),
            base_url: base_url.into().trim_end_matches('/').to_string(),
            token: None,
        }
    }

    /// A copy of this client that sends `token` as a bearer token.
    pub fn with_token(&self, token: impl Into<String>) -> Self {
        Self {
            token: Some(token.into()),
            ..self.clone()
        }
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub async fn register(&self, username: &str, password: &str) -> Result<User, ApiError> {
        let req = self
            .http
            .post(self.url("/auth/register"))
            .json(&Credentials { username, password });
        json(self.send(req).await?).await
    }

    pub async fn login(&self, username: &str, password: &str) -> Result<Token, ApiError> {
        let req = self
            .http
            .post(self.url("/auth/login"))
            .json(&Credentials { username, password });
        json(self.send(req).await?).await
    }

    /// One page of the caller's todos, oldest first.
    pub async fn list_todos(&self, limit: u32, offset: u32) -> Result<Vec<Todo>, ApiError> {
        let req = self
            .http
            .get(self.url(&format!("/todos?limit={limit}&offset={offset}")));
        json(self.send(req).await?).await
    }

    /// All of the caller's todos, fetched page by page.
    pub async fn all_todos(&self) -> Result<Vec<Todo>, ApiError> {
        let mut todos = Vec::new();
        loop {
            let page = self.list_todos(MAX_PAGE_SIZE, todos.len() as u32).await?;
            let last = page.len() < MAX_PAGE_SIZE as usize;
            todos.extend(page);
            if last {
                return Ok(todos);
            }
        }
    }

    pub async fn create_todo(&self, todo: &NewTodo) -> Result<Todo, ApiError> {
        let req = self.http.post(self.url("/todos")).json(todo);
        json(self.send(req).await?).await
    }

    pub async fn update_todo(&self, id: &str, changes: &TodoChanges) -> Result<Todo, ApiError> {
        let req = self
            .http
            .patch(self.url(&format!("/todos/{id}")))
            .json(changes);
        json(self.send(req).await?).await
    }

    pub async fn delete_todo(&self, id: &str) -> Result<(), ApiError> {
        let req = self.http.delete(self.url(&format!("/todos/{id}")));
        self.send(req).await.map(drop)
    }

    fn url(&self, path: &str) -> String {
        format!("{}{path}", self.base_url)
    }

    /// Sends the request with the bearer token, turning error statuses into [`ApiError`].
    async fn send(&self, req: RequestBuilder) -> Result<Response, ApiError> {
        let req = match &self.token {
            Some(token) => req.bearer_auth(token),
            None => req,
        };
        let res = req.send().await.map_err(transport)?;

        let status = res.status();
        if status.is_success() {
            return Ok(res);
        }
        if status == StatusCode::UNAUTHORIZED {
            return Err(ApiError::Unauthorized);
        }

        let message = match res.json::<ErrorBody>().await {
            Ok(body) => body.error,
            Err(_) => status
                .canonical_reason()
                .unwrap_or("request failed")
                .to_lowercase(),
        };
        Err(ApiError::Status {
            status: status.as_u16(),
            message,
        })
    }
}

async fn json<T: DeserializeOwned>(res: Response) -> Result<T, ApiError> {
    res.json().await.map_err(transport)
}

fn transport(err: reqwest::Error) -> ApiError {
    ApiError::Transport(err.to_string())
}
