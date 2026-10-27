//! API client tests. The serialization tests need nothing; the live tests talk to a running
//! todo-backend (`TODO_API_URL`, default http://localhost:3000) and are `#[ignore]`d:
//! `cargo test -- --ignored`.

use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::json;
use todo_frontend::api::{ApiError, Client, NewTodo, Todo, TodoChanges, base_url_from_env};

#[test]
fn empty_changes_serialize_to_empty_object() {
    let changes = TodoChanges::default();
    assert!(changes.is_empty());
    assert_eq!(serde_json::to_value(&changes).unwrap(), json!({}));
}

#[test]
fn clearing_description_sends_null() {
    let changes = TodoChanges {
        description: Some(None),
        completed: Some(true),
        ..Default::default()
    };
    assert!(!changes.is_empty());
    assert_eq!(
        serde_json::to_value(&changes).unwrap(),
        json!({"description": null, "completed": true})
    );
}

#[test]
fn new_todo_omits_missing_description() {
    let todo = NewTodo {
        title: "Buy milk".into(),
        description: None,
    };
    assert_eq!(
        serde_json::to_value(&todo).unwrap(),
        json!({"title": "Buy milk"})
    );
}

#[test]
fn todo_deserializes_from_backend_shape() {
    let todo: Todo = serde_json::from_value(json!({
        "id": "74b182dc-c748-4f50-a1f6-d394f8705bf3",
        "title": "Buy milk",
        "description": null,
        "completed": false,
        "created_at": "2026-10-27T22:18:24.491487Z",
        "updated_at": "2026-10-27T22:18:24.491487Z"
    }))
    .unwrap();
    assert_eq!(todo.title, "Buy milk");
    assert_eq!(todo.description, None);
    assert!(!todo.completed);
}

#[test]
fn client_trims_trailing_slash() {
    assert_eq!(
        Client::new("http://example.test/").base_url(),
        "http://example.test"
    );
}

fn unique_username() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    format!("fe_{}", nanos % 10_u128.pow(20))
}

async fn logged_in_client() -> Client {
    let client = Client::new(base_url_from_env());
    let username = unique_username();
    client.register(&username, "correct-horse").await.unwrap();
    let token = client.login(&username, "correct-horse").await.unwrap();
    client.with_token(token.token)
}

#[tokio::test]
#[ignore = "needs a running todo-backend"]
async fn auth_errors() {
    let client = Client::new(base_url_from_env());
    let username = unique_username();
    client.register(&username, "correct-horse").await.unwrap();

    let err = client
        .register(&username, "correct-horse")
        .await
        .unwrap_err();
    assert!(
        matches!(err, ApiError::Status { status: 409, .. }),
        "{err:?}"
    );

    let err = client.login(&username, "wrong-password").await.unwrap_err();
    assert_eq!(err, ApiError::Unauthorized);

    let err = client.all_todos().await.unwrap_err();
    assert_eq!(err, ApiError::Unauthorized);
}

#[tokio::test]
#[ignore = "needs a running todo-backend"]
async fn todo_lifecycle() {
    let client = logged_in_client().await;
    assert_eq!(client.all_todos().await.unwrap(), vec![]);

    let created = client
        .create_todo(&NewTodo {
            title: "  Buy milk  ".into(),
            description: Some("2 litres".into()),
        })
        .await
        .unwrap();
    assert_eq!(created.title, "Buy milk");
    assert_eq!(created.description.as_deref(), Some("2 litres"));

    let changes = TodoChanges {
        description: Some(None),
        completed: Some(true),
        ..Default::default()
    };
    let updated = client.update_todo(&created.id, &changes).await.unwrap();
    assert_eq!(updated.description, None);
    assert!(updated.completed);
    assert_eq!(client.all_todos().await.unwrap(), vec![updated]);

    let err = client
        .create_todo(&NewTodo {
            title: "   ".into(),
            description: None,
        })
        .await
        .unwrap_err();
    assert!(
        matches!(err, ApiError::Status { status: 422, .. }),
        "{err:?}"
    );

    client.delete_todo(&created.id).await.unwrap();
    let err = client.delete_todo(&created.id).await.unwrap_err();
    assert!(
        matches!(err, ApiError::Status { status: 404, .. }),
        "{err:?}"
    );
    assert_eq!(client.all_todos().await.unwrap(), vec![]);
}

#[tokio::test]
#[ignore = "needs a running todo-backend"]
async fn all_todos_reads_every_page() {
    let client = logged_in_client().await;
    for i in 0..105 {
        client
            .create_todo(&NewTodo {
                title: format!("todo {i}"),
                description: None,
            })
            .await
            .unwrap();
    }

    let first = client.list_todos(100, 0).await.unwrap();
    assert_eq!(first.len(), 100);

    let all = client.all_todos().await.unwrap();
    assert_eq!(all.len(), 105);
    assert_eq!(all[0].title, "todo 0");
    assert_eq!(all[104].title, "todo 104");
}
