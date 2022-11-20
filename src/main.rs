mod factorial;

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use anyhow::{Ok, Result};
use async_graphql::{
    http::{playground_source, GraphQLPlaygroundConfig},
    EmptyMutation, EmptySubscription, MergedObject, Object, Request, Response, Schema,
};
use axum::{
    response::{Html, IntoResponse},
    routing::get,
    Extension, Json, Router,
};
use bb8_redis::RedisConnectionManager;
use factorial::{async_operation, calculate_factorial, calculate_factorial_py};
use pyo3::{PyResult, Python};
use redis::AsyncCommands;
use tokio::sync::Mutex;

#[derive(Default)]
struct GenericQueryRoot;

#[Object]
impl GenericQueryRoot {
    async fn hello(&self, name: String) -> String {
        format!("Hello {name}")
    }

    async fn factorial(&self, number: usize) -> String {
        let start = Instant::now();
        let fac = calculate_factorial(number);
        println!("Time elapsed rs: {}", start.elapsed().as_micros());
        let start = Instant::now();
        let res = calculate_factorial_py(number);
        println!(
            "Time elapsed py: {}, res: {:?}",
            start.elapsed().as_micros(),
            res
        );
        format!("{fac}")
    }
}

#[derive(Default)]
struct ParsedQueryRoot;

#[Object]
impl ParsedQueryRoot {
    async fn parse(&self, input: String) -> Result<i32> {
        let data: i32 = input.parse()?;
        Ok(data)
    }
}

#[derive(MergedObject, Default)]
struct QueryRoot(GenericQueryRoot, ParsedQueryRoot);

type TestingSchema = Schema<QueryRoot, EmptyMutation, EmptySubscription>;

async fn graphql_handler(schema: Extension<TestingSchema>, req: Json<Request>) -> Json<Response> {
    schema.execute(req.0).await.into()
}

async fn graphql_playground() -> impl IntoResponse {
    Html(playground_source(GraphQLPlaygroundConfig::new("/graphql")))
}

async fn sleep() -> &'static str {
    async_operation().await;
    "Success"
}

async fn rust_sleep() -> &'static str {
    tokio::time::sleep(Duration::from_millis(10)).await;
    "Success"
}

async fn get_from_redis(connection: Extension<Arc<Mutex<redis::aio::Connection>>>) -> String {
    let val: String = connection.lock().await.get("Testing Key").await.unwrap();
    val
}

async fn get_from_redis_pool(
    pool: Extension<bb8_redis::bb8::Pool<RedisConnectionManager>>,
) -> String {
    let mut conn = pool.get().await.unwrap();
    let val: String = conn.get("Testing Key").await.unwrap();
    val
}

#[pyo3_asyncio::tokio::main(flavor = "multi_thread")]
async fn main() -> PyResult<()> {
    pyo3::prepare_freethreaded_python();
    let client = redis::Client::open("redis://:@0.0.0.0:6379/3").unwrap();
    let connection = client.get_async_connection().await.unwrap();
    let schema: TestingSchema =
        Schema::build(QueryRoot::default(), EmptyMutation, EmptySubscription).finish();
    let locals = Python::with_gil(pyo3_asyncio::tokio::get_current_locals)?;
    let manager = RedisConnectionManager::new("redis://:@0.0.0.0:6379/3").unwrap();
    let pool = bb8_redis::bb8::Pool::builder()
        .build(manager)
        .await
        .unwrap();
    let app = Router::new()
        .route("/graphql", get(graphql_playground).post(graphql_handler))
        .route(
            "/sleep",
            get(move || pyo3_asyncio::tokio::scope(locals.clone(), sleep())),
        )
        .route("/rust_sleep", get(rust_sleep))
        .route("/redis_get", get(get_from_redis))
        .route("/redis_pool_get", get(get_from_redis_pool))
        .layer(Extension(schema))
        .layer(Extension(pool))
        .layer(Extension(Arc::new(Mutex::new(connection))));

    axum::Server::bind(&"0.0.0.0:3000".parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();
    PyResult::Ok(())
}
