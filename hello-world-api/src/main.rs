use anyhow::Context;
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Extension, Json, Router,
};
use serde::{Deserialize, Serialize};

use sqlx::{postgres::PgPoolOptions, PgPool};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // initialize tracing
    tracing_subscriber::fmt::init();

    let database_url = "postgres://postgres:postgres@localhost:5432/postgres";
    let db = PgPoolOptions::new()
        .max_connections(20)
        .connect(database_url)
        .await
        .context("failed to connect to DATABASE_URL")?;

    sqlx::migrate!()
        .run(&db)
        .await
        .context("failed to migrate")?;

    // build our application with a route
    let app = Router::new()
        // `GET /` goes to `root`
        .route("/", get(root))
        // `POST /users` goes to `create_user`
        .route("/users", post(create_user))
        .route("/users", get(get_users))
        .layer(Extension(db));

    axum::Server::bind(
        &"0.0.0.0:3000"
            .parse()
            .context("Unable to bind to port 3000")?,
    )
    .serve(app.into_make_service())
    .await
    .context("Unable to start server")?;

    Ok(())
}

// basic handler that responds with a static string
async fn root() -> &'static str {
    "Hello, World!"
}

async fn create_user(
    // this argument tells axum to parse the request body
    // as JSON into a `CreateUser` type
    db: Extension<PgPool>,
    Json(payload): Json<CreateUser>,
) -> Response {
    let res = sqlx::query_as::<_, User>(
        r#"
            INSERT INTO "user"(user_id, username)
            VALUES (DEFAULT, $1)
            RETURNING user_id, username;
        "#,
    )
    .bind(payload.username)
    .fetch_one(&*db)
    .await;

    match res {
        Err(e) => match e {
            sqlx::Error::Database(dbe) if dbe.constraint() == Some("user_username_key") => (
                StatusCode::BAD_REQUEST,
                Json(ApiError {
                    error: "Username already exists".to_string(),
                }),
            )
                .into_response(),
            _ => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError {
                    error: format!("Error: {0:?}", e),
                }),
            )
                .into_response(),
        },
        Ok(u) => (StatusCode::CREATED, Json(u)).into_response(),
    }
}

async fn get_users(db: Extension<PgPool>) -> Response {
    let res = sqlx::query_as::<_, User>(
        r#"
            SELECT user_id, username
            FROM "user";
        "#,
    )
    .fetch_all(&*db)
    .await;

    match res {
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError {
                error: format!("Error: {0:?}", e),
            }),
        )
            .into_response(),
        Ok(u) => (StatusCode::OK, Json(u)).into_response(),
    }
}

// the input to our `create_user` handler
#[derive(Deserialize)]
struct CreateUser {
    username: String,
}

// the output to our `create_user` handler
#[derive(Serialize, sqlx::FromRow)]
struct User {
    user_id: sqlx::types::Uuid,
    username: String,
}

#[derive(Serialize)]
struct ApiError {
    error: String,
}
