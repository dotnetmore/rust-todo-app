use std::time::Duration;

use anyhow::Context;
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Extension, Json, Router,
};
use serde::{Deserialize, Serialize};

use sqlx::{postgres::PgPoolOptions, PgPool};
use tracing::{event, info, Level};

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
        // `GET /` goes to `root`
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
#[derive(Serialize)]
struct ApiError {
    error: String,
}
