use std::time::Duration;

use anyhow::Context;
use axum::{
    debug_handler,
    extract::Path,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post, put},
    Extension, Json, Router,
};
use serde::{Deserialize, Serialize};

use sqlx::{error::DatabaseError, postgres::PgPoolOptions, PgPool};
use tracing::{error, event, info, Level};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(Level::DEBUG)
        .init();

    let database_url = "postgres://postgres:postgres@127.0.0.1:5432/postgres";
    let db = PgPoolOptions::new()
        .max_connections(20)
        .acquire_timeout(Duration::from_millis(500))
        .connect(database_url)
        .await
        .context("failed to connect to DATABASE_URL")?;

    sqlx::migrate!()
        .run(&db)
        .await
        .context("failed to migrate")?;

    info!("Database migrated!");

    // build our application with a route
    let app = Router::new()
        .route("/todos", get(get_todos).post(create_todo))
        .route("/todos/:id", get(get_todo))
        .route("/todos/:id", put(put_todo_done))
        .layer(Extension(db))
        .layer(tower_http::trace::TraceLayer::new_for_http());

    axum::Server::bind(&"0.0.0.0:3000".parse().context("Unable to parse to port")?)
        .serve(app.into_make_service())
        .await
        .context("Unable to start server")?;

    Ok(())
}

async fn get_todos(pg: Extension<PgPool>) -> axum::response::Response {
    let result = sqlx::query_as::<_, Todo>(
        r#"select id, todo_text, is_done from "todo" order by id limit $1"#,
    )
    .bind(10)
    .fetch_all(&*pg)
    .await;
    match result {
        Result::Ok(todos) => (
            StatusCode::OK,
            Json(todos.iter().map(ToDoView::from).collect::<Vec<ToDoView>>()),
        )
            .into_response(),
        Err(err) => ApiError::from(err).into_response(),
    }
}

async fn get_todo(pg: Extension<PgPool>, Path(id): Path<uuid::Uuid>) -> axum::response::Response {
    let result =
        sqlx::query_as::<_, Todo>(r#"select id, todo_text, is_done from "todo" where id = $1"#)
            .bind(id)
            .fetch_one(&*pg)
            .await;
    match result {
        Result::Ok(todo) => (StatusCode::OK, Json(ToDoView::from(todo))).into_response(),
        Err(err) => ApiError::from(err).into_response(),
    }
}

#[debug_handler]
async fn put_todo_done(
    pg: Extension<PgPool>,
    Path(id): Path<uuid::Uuid>,
    axum::extract::Json(body): axum::extract::Json<PutTodo>,
) -> axum::response::Response {
    let result = sqlx::query_as::<_, Todo>(
        r#"update "todo" set is_done = $1 where id = $2 returning id, todo_text, is_done"#,
    )
    .bind(body.is_done)
    .bind(id)
    .fetch_one(&*pg)
    .await;
    match result {
        Result::Ok(todo) => (StatusCode::OK, Json(ToDoView::from(todo))).into_response(),
        Err(err) => ApiError::from(err).into_response(),
    }
}

async fn create_todo(
    pg: Extension<PgPool>,
    axum::extract::Json(body): axum::extract::Json<CreateTodo>,
) -> axum::response::Response {
    let result = sqlx::query_as::<_, Todo>(
        r#"insert into "todo" (todo_text) values ($1) returning id, todo_text, is_done"#,
    )
    .bind(body.text)
    .fetch_one(&*pg)
    .await;
    match result {
        Result::Ok(todo) => (StatusCode::CREATED, Json(ToDoView::from(todo))).into_response(),
        Err(err) => ApiError::from(err).into_response(),
    }
}

#[derive(sqlx::FromRow)]
struct Todo {
    id: uuid::Uuid,
    todo_text: String,
    is_done: bool,
}

#[derive(Deserialize)]
struct CreateTodo {
    text: String,
}

#[derive(Serialize)]
struct ToDoView {
    id: uuid::Uuid,
    text: String,
    is_done: bool,
}

impl From<&Todo> for ToDoView {
    fn from(todo: &Todo) -> Self {
        ToDoView {
            id: todo.id,
            text: todo.todo_text.clone(),
            is_done: todo.is_done,
        }
    }
}

impl From<Todo> for ToDoView {
    fn from(todo: Todo) -> Self {
        ToDoView {
            id: todo.id,
            text: todo.todo_text,
            is_done: todo.is_done,
        }
    }
}

struct ApiError {
    code: StatusCode,
    error: String,
}

impl From<Box<dyn DatabaseError>> for ApiError {
    fn from(value: Box<dyn DatabaseError>) -> Self {
        if let Some(code) = value.code() {
            if code == "23505" {
                return ApiError {
                    code: StatusCode::CONFLICT,
                    error: format!("Duplicate entity").to_owned(),
                };
            }
        }
        ApiError {
            code: StatusCode::INTERNAL_SERVER_ERROR,
            error: format!("{:?}", value).to_owned(),
        }
    }
}

impl From<sqlx::Error> for ApiError {
    fn from(err: sqlx::Error) -> Self {
        match err {
            sqlx::Error::Database(db_err) => db_err.into(),
            sqlx::Error::RowNotFound => ApiError {
                code: StatusCode::NOT_FOUND,
                error: "Not found".to_owned(),
            },
            _ => {
                error!("Fail to insert into database {:?}", err);
                ApiError {
                    code: StatusCode::INTERNAL_SERVER_ERROR,
                    error: "Fail to insert into database".to_owned(),
                }
            }
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.code, self.error).into_response()
    }
}

#[derive(Deserialize)]
struct PutTodo {
    is_done: bool,
}
