//! Dioxus components: the login/register screen and the todo list.
//!
//! Text inputs are deliberately uncontrolled: `oninput` mirrors their value into a signal, but
//! the signal is never bound back with `value:`. A bound value is written back on every render,
//! and in the desktop webview that overwrites keystrokes typed before the render catches up,
//! dropping characters when typing fast. Edit fields are prefilled with `initial_value`.

use dioxus::prelude::*;

use crate::api::{ApiError, Client, NewTodo, Todo, TodoChanges, base_url_from_env};

const CSS: &str = include_str!("../assets/main.css");

/// A logged-in user and a client that carries their token.
#[derive(Clone)]
struct Session {
    username: String,
    client: Client,
}

/// App-wide state shared through context. Signals are `Copy`, so this is too.
#[derive(Clone, Copy)]
struct AppState {
    /// Unauthenticated client for login and registration.
    anonymous: Signal<Client>,
    session: Signal<Option<Session>>,
    /// Message shown on the login screen, e.g. after the session expired.
    notice: Signal<Option<String>>,
}

impl AppState {
    fn client(&self) -> Client {
        match &*self.session.read() {
            Some(session) => session.client.clone(),
            None => self.anonymous.read().clone(),
        }
    }

    fn log_out(mut self, notice: Option<&str>) {
        self.session.set(None);
        self.notice.set(notice.map(str::to_string));
    }

    /// Ends the session on `401`; otherwise shows the error in `target`.
    fn report(self, mut target: Signal<Option<String>>, err: ApiError) {
        match err {
            ApiError::Unauthorized => {
                self.log_out(Some("Your session has expired. Please log in again."))
            }
            err => target.set(Some(err.to_string())),
        }
    }
}

#[component]
pub fn App() -> Element {
    let state = use_context_provider(|| AppState {
        anonymous: Signal::new(Client::new(base_url_from_env())),
        session: Signal::new(None),
        notice: Signal::new(None),
    });

    rsx! {
        style { {CSS} }
        main {
            if state.session.read().is_some() {
                TodoScreen {}
            } else {
                AuthScreen {}
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum AuthMode {
    Login,
    Register,
}

#[component]
fn AuthScreen() -> Element {
    let mut state = use_context::<AppState>();
    let mut mode = use_signal(|| AuthMode::Login);
    let mut username = use_signal(String::new);
    let mut password = use_signal(String::new);
    let mut error = use_signal(|| None::<String>);
    let mut busy = use_signal(|| false);

    let submit = move |evt: FormEvent| async move {
        evt.prevent_default();
        if busy() {
            return;
        }
        busy.set(true);
        error.set(None);
        state.notice.set(None);

        let client = state.client();
        let (name, pass) = (username(), password());
        let result = async {
            if mode() == AuthMode::Register {
                client.register(&name, &pass).await?;
            }
            client.login(&name, &pass).await
        }
        .await;

        match result {
            Ok(token) => state.session.set(Some(Session {
                username: name,
                client: client.with_token(token.token),
            })),
            Err(ApiError::Unauthorized) => {
                error.set(Some("Wrong username or password.".to_string()))
            }
            Err(err) => error.set(Some(err.to_string())),
        }
        busy.set(false);
    };

    let (heading, action, switch_prompt, switch_action) = match mode() {
        AuthMode::Login => ("Log in", "Log in", "No account yet?", "Register"),
        AuthMode::Register => (
            "Create an account",
            "Register",
            "Already have an account?",
            "Log in",
        ),
    };

    rsx! {
        section { class: "card auth",
            h1 { "Todos" }
            h2 { {heading} }
            if let Some(notice) = state.notice.read().as_ref() {
                p { class: "notice", {notice.clone()} }
            }
            if let Some(err) = error.read().as_ref() {
                p { class: "error", {err.clone()} }
            }
            form { onsubmit: submit,
                label {
                    "Username"
                    input {
                        r#type: "text",
                        onmounted: focus,
                        autocomplete: "username",
                        oninput: move |e| username.set(e.value()),
                    }
                }
                label {
                    "Password"
                    input {
                        r#type: "password",
                        autocomplete: if mode() == AuthMode::Login { "current-password" } else { "new-password" },
                        oninput: move |e| password.set(e.value()),
                    }
                }
                if mode() == AuthMode::Register {
                    p { class: "hint",
                        "Username: 3–32 letters, digits, _ or -. Password: 8–128 characters."
                    }
                }
                button {
                    r#type: "submit",
                    class: "primary",
                    disabled: busy() || username.read().is_empty() || password.read().is_empty(),
                    {action}
                }
            }
            p { class: "switch",
                "{switch_prompt} "
                button {
                    class: "link",
                    onclick: move |_| {
                        mode.set(if mode() == AuthMode::Login { AuthMode::Register } else { AuthMode::Login });
                        error.set(None);
                    },
                    {switch_action}
                }
            }
            p { class: "server", "Server: {state.anonymous.read().base_url()}" }
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Filter {
    All,
    Active,
    Completed,
}

impl Filter {
    fn matches(self, todo: &Todo) -> bool {
        match self {
            Filter::All => true,
            Filter::Active => !todo.completed,
            Filter::Completed => todo.completed,
        }
    }
}

/// State of the todo list, shared with its items through context.
#[derive(Clone, Copy)]
struct ListState {
    todos: Signal<Vec<Todo>>,
    error: Signal<Option<String>>,
}

impl ListState {
    fn replace(mut self, updated: Todo) {
        if let Some(todo) = self.todos.write().iter_mut().find(|t| t.id == updated.id) {
            *todo = updated;
        }
    }

    fn remove(mut self, id: &str) {
        self.todos.write().retain(|t| t.id != id);
    }
}

#[component]
fn TodoScreen() -> Element {
    let state = use_context::<AppState>();
    let list = use_context_provider(|| ListState {
        todos: Signal::new(Vec::new()),
        error: Signal::new(None),
    });
    let mut list_signals = list;
    let mut loading = use_signal(|| true);
    let mut filter = use_signal(|| Filter::All);

    let reload = move || {
        spawn(async move {
            loading.set(true);
            match state.client().all_todos().await {
                Ok(todos) => {
                    list_signals.todos.set(todos);
                    list_signals.error.set(None);
                }
                Err(err) => state.report(list.error, err),
            }
            loading.set(false);
        });
    };
    use_hook(reload);

    let username = state
        .session
        .read()
        .as_ref()
        .map(|s| s.username.clone())
        .unwrap_or_default();
    let todos = list.todos.read();
    let total = todos.len();
    let active = todos.iter().filter(|t| !t.completed).count();
    let visible: Vec<Todo> = todos
        .iter()
        .filter(|t| filter().matches(t))
        .cloned()
        .collect();

    rsx! {
        header {
            h1 { "Todos" }
            div { class: "account",
                span { "{username}" }
                button { onclick: move |_| reload(), disabled: loading(), "Refresh" }
                button { onclick: move |_| state.log_out(None), "Log out" }
            }
        }
        if let Some(err) = list.error.read().as_ref() {
            p { class: "error",
                {err.clone()}
                button {
                    class: "link dismiss",
                    onclick: move |_| list_signals.error.set(None),
                    "Dismiss"
                }
            }
        }
        NewTodoForm {}
        nav { class: "filters",
            for (f , label , count) in [
                (Filter::All, "All", total),
                (Filter::Active, "Active", active),
                (Filter::Completed, "Completed", total - active),
            ]
            {
                button {
                    class: if filter() == f { "selected" } else { "" },
                    onclick: move |_| filter.set(f),
                    "{label} ({count})"
                }
            }
        }
        if loading() && total == 0 {
            p { class: "empty", "Loading…" }
        } else if visible.is_empty() {
            p { class: "empty",
                match filter() {
                    Filter::All => "Nothing to do. Add your first todo above.",
                    Filter::Active => "No open todos.",
                    Filter::Completed => "Nothing completed yet.",
                }
            }
        } else {
            ul { class: "todos",
                for todo in visible {
                    TodoItem { key: "{todo.id}", todo: todo.clone() }
                }
            }
        }
    }
}

#[component]
fn NewTodoForm() -> Element {
    let state = use_context::<AppState>();
    let mut list = use_context::<ListState>();
    let mut title = use_signal(String::new);
    let mut description = use_signal(String::new);
    let mut busy = use_signal(|| false);

    let submit = move |evt: FormEvent| async move {
        evt.prevent_default();
        if busy() || title.read().trim().is_empty() {
            return;
        }
        busy.set(true);
        let new = NewTodo {
            title: title(),
            description: non_empty(&description.read()),
        };
        match state.client().create_todo(&new).await {
            Ok(todo) => {
                list.todos.write().push(todo);
                list.error.set(None);
                title.set(String::new());
                description.set(String::new());
                // The inputs are uncontrolled (see the module docs), so clear them in the DOM.
                document::eval("document.getElementById('new-todo').reset()");
            }
            Err(err) => state.report(list.error, err),
        }
        busy.set(false);
    };

    rsx! {
        form { id: "new-todo", class: "card new-todo", onsubmit: submit,
            input {
                r#type: "text",
                placeholder: "What needs doing?",
                onmounted: focus,
                maxlength: 200,
                oninput: move |e| title.set(e.value()),
            }
            textarea {
                placeholder: "Description (optional)",
                rows: 2,
                maxlength: 2000,
                oninput: move |e| description.set(e.value()),
            }
            button {
                r#type: "submit",
                class: "primary",
                disabled: busy() || title.read().trim().is_empty(),
                "Add"
            }
        }
    }
}

#[component]
fn TodoItem(todo: Todo) -> Element {
    let state = use_context::<AppState>();
    let list = use_context::<ListState>();
    let mut busy = use_signal(|| false);
    let mut editing = use_signal(|| false);
    let mut title = use_signal(String::new);
    let mut description = use_signal(String::new);

    let id = todo.id.clone();
    let completed = todo.completed;
    let toggle = move |_| {
        let id = id.clone();
        async move {
            busy.set(true);
            let changes = TodoChanges {
                completed: Some(!completed),
                ..Default::default()
            };
            match state.client().update_todo(&id, &changes).await {
                Ok(updated) => list.replace(updated),
                Err(err) => state.report(list.error, err),
            }
            busy.set(false);
        }
    };

    let id = todo.id.clone();
    let delete = move |_| {
        let id = id.clone();
        async move {
            busy.set(true);
            match state.client().delete_todo(&id).await {
                Ok(()) => list.remove(&id),
                Err(err) => state.report(list.error, err),
            }
            busy.set(false);
        }
    };

    let current = todo.clone();
    let start_edit = move |_| {
        title.set(current.title.clone());
        description.set(current.description.clone().unwrap_or_default());
        editing.set(true);
    };

    let current = todo.clone();
    let save = move |evt: FormEvent| {
        evt.prevent_default();
        let current = current.clone();
        async move {
            let changes = edit_changes(&current, &title.read(), &description.read());
            if changes.is_empty() {
                editing.set(false);
                return;
            }
            busy.set(true);
            match state.client().update_todo(&current.id, &changes).await {
                Ok(updated) => {
                    list.replace(updated);
                    editing.set(false);
                }
                Err(err) => state.report(list.error, err),
            }
            busy.set(false);
        }
    };

    if editing() {
        return rsx! {
            li { class: "todo editing",
                form {
                    onsubmit: save,
                    onkeydown: move |e| {
                        if e.key() == Key::Escape {
                            editing.set(false);
                        }
                    },
                    input {
                        r#type: "text",
                        onmounted: focus,
                        maxlength: 200,
                        initial_value: "{title}",
                        oninput: move |e| title.set(e.value()),
                    }
                    textarea {
                        placeholder: "Description (optional)",
                        rows: 3,
                        maxlength: 2000,
                        initial_value: "{description}",
                        oninput: move |e| description.set(e.value()),
                    }
                    div { class: "actions",
                        button {
                            r#type: "button",
                            disabled: busy(),
                            onclick: move |_| editing.set(false),
                            "Cancel"
                        }
                        button {
                            r#type: "submit",
                            class: "primary",
                            disabled: busy() || title.read().trim().is_empty(),
                            "Save"
                        }
                    }
                }
            }
        };
    }

    rsx! {
        li { class: if todo.completed { "todo done" } else { "todo" },
            input {
                r#type: "checkbox",
                checked: todo.completed,
                disabled: busy(),
                onchange: toggle,
            }
            div { class: "body", ondoubleclick: start_edit.clone(),
                span { class: "title", "{todo.title}" }
                if let Some(description) = &todo.description {
                    p { class: "description", "{description}" }
                }
            }
            div { class: "actions",
                button { disabled: busy(), onclick: start_edit, "Edit" }
                button { class: "danger", disabled: busy(), onclick: delete, "Delete" }
            }
        }
    }
}

/// The fields that an edit actually changed. A blank description clears it.
fn edit_changes(current: &Todo, title: &str, description: &str) -> TodoChanges {
    let title = title.trim();
    let description = non_empty(description);
    TodoChanges {
        title: (title != current.title).then(|| title.to_string()),
        description: (description != current.description).then_some(description),
        completed: None,
    }
}

/// Focuses an input when it's mounted. Unlike `autofocus`, this also works for inputs that
/// appear after the page has loaded, such as the edit form.
async fn focus(evt: MountedEvent) {
    let _ = evt.set_focus(true).await;
}

fn non_empty(text: &str) -> Option<String> {
    let text = text.trim();
    (!text.is_empty()).then(|| text.to_string())
}
