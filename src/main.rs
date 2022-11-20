mod factorial;

use std::time::{Duration, Instant};

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
use factorial::{async_operation, calculate_factorial, calculate_factorial_py};
use pyo3::{PyResult, Python};

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

#[pyo3_asyncio::tokio::main(flavor = "multi_thread")]
async fn main() -> PyResult<()> {
    pyo3::prepare_freethreaded_python();
    let schema: TestingSchema =
        Schema::build(QueryRoot::default(), EmptyMutation, EmptySubscription).finish();
    let locals = Python::with_gil(pyo3_asyncio::tokio::get_current_locals)?;
    let app = Router::new()
        .route("/graphql", get(graphql_playground).post(graphql_handler))
        .route(
            "/sleep",
            get(move || pyo3_asyncio::tokio::scope(locals.clone(), sleep())),
        )
        .route("/rust_sleep", get(rust_sleep))
        .layer(Extension(schema));

    axum::Server::bind(&"0.0.0.0:3000".parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();
    PyResult::Ok(())
}
