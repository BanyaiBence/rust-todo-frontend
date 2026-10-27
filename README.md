# todo-frontend

A desktop client for the todo-backend JSON API, written in Rust with [Dioxus](https://dioxuslabs.com/) 0.7.

Features:

- Register and log in with a username and password
- Create todos with an optional description
- Edit, complete and delete todos
- Filter the list by All / Active / Completed
- Automatic logout with a notice when the session token expires (`401`)

## Requirements

- Rust with edition 2024 support (1.85 or newer)
- A running [todo-backend](#backend) instance
- On Linux, the system libraries Dioxus desktop needs (WebKitGTK and friends). On Fedora, for example:

  ```sh
  sudo dnf install webkit2gtk4.1-devel libxdo-devel openssl-devel
  ```

  See the [Dioxus getting-started guide](https://dioxuslabs.com/learn/0.7/getting_started) for other platforms.

## Running

```sh
cargo run
```

The app talks to `http://localhost:3000` by default. Point it at another backend with `TODO_API_URL`:

```sh
TODO_API_URL=http://todo.example.com cargo run
```

## Make targets

A `Makefile` wraps the common Cargo commands. Run `make` (or `make help`) to list them:

| Target             | What it does                                       |
| ------------------ | -------------------------------------------------- |
| `make build`       | Debug build                                        |
| `make release`     | Optimized release build (`target/release/`)        |
| `make run`         | Run the app; `make run-release` for a release run  |
| `make test`        | Tests that need no backend                         |
| `make test-live`   | Only the live tests against `TODO_API_URL`         |
| `make test-all`    | All tests, including the live ones                 |
| `make clippy`      | Clippy with warnings treated as errors             |
| `make fmt`         | Format the code; `make fmt-check` only checks      |
| `make check`       | `fmt-check`, `clippy` and `test` (also `make ci`)  |
| `make clean`       | Remove build artifacts                             |

`TODO_API_URL` can be passed to any target, e.g. `make run TODO_API_URL=http://todo.example.com`.

## Backend

The client expects these endpoints:

| Method   | Path             | Purpose                                    |
| -------- | ---------------- | ------------------------------------------ |
| `POST`   | `/auth/register` | Create a user                              |
| `POST`   | `/auth/login`    | Get a bearer token                         |
| `GET`    | `/todos`         | List todos (`limit` ≤ 100, `offset`)       |
| `POST`   | `/todos`         | Create a todo                              |
| `PATCH`  | `/todos/{id}`    | Update title, description and/or completed |
| `DELETE` | `/todos/{id}`    | Delete a todo                              |

Authenticated requests send `Authorization: Bearer <token>`. Error responses are expected as `{"error": "<message>"}`.

## Testing

```sh
# Serialization tests, no backend needed
cargo test

# Also run the live tests against a backend at TODO_API_URL (default http://localhost:3000)
cargo test -- --ignored
```

## Project layout

```
src/
  main.rs   Window setup and app launch
  api.rs    Typed async client for the backend (reqwest + serde)
  app.rs    Dioxus components: auth screen, todo list, new-todo form, todo item
assets/
  main.css  Styles, embedded into the binary at compile time
tests/
  api.rs    API client tests
```
