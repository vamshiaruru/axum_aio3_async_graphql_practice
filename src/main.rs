mod entities;
mod factorial;

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use async_graphql::{
    http::{playground_source, GraphQLPlaygroundConfig},
    Context, EmptyMutation, EmptySubscription, MergedObject, Object, Request, Response, Schema,
};
use axum::{
    response::{Html, IntoResponse},
    routing::get,
    Extension, Router,
};
use bb8_redis::RedisConnectionManager;
use entities::page::{self};
use factorial::{async_operation, calculate_factorial, calculate_factorial_py};
use pyo3::{PyResult, Python};
use rand::Rng;
use redis::AsyncCommands;
use sea_orm::{prelude::*, Database, FromQueryResult, QuerySelect};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

type RedisBb8ConnectionPool = bb8_redis::bb8::Pool<RedisConnectionManager>;

#[derive(Default)]
struct GenericQueryRoot;

#[derive(FromQueryResult, Debug, Serialize, Deserialize)]
struct PageSlug {
    slug: String,
}

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

    async fn page(&self, ctx: &Context<'_>) -> Result<Option<page::Model>, DbErr> {
        let num = rand::thread_rng().gen_range(1..100000);
        let db = ctx.data::<DatabaseConnection>().unwrap();
        entities::prelude::Page::find_by_id(num).one(db).await
    }

    async fn page_from_redis(&self, ctx: &Context<'_>) -> anyhow::Result<page::Model> {
        let num = rand::thread_rng().gen_range(1..100000);
        let mut conn = ctx
            .data::<RedisBb8ConnectionPool>()
            .expect("Couldn't get redis pool")
            .get()
            .await?;
        let val: String = conn.get(format!("slug_{num}")).await?;
        let data: page::Model = serde_json::from_str(&val)?;
        anyhow::Ok(data)
    }

    async fn page_from_query(&self, ctx: &Context<'_>, id: i32) -> anyhow::Result<page::Model> {
        let db = ctx.data::<DatabaseConnection>().unwrap();
        let res = page::Entity::find()
            .select_only()
            .column(page::Column::Slug)
            .filter(page::Column::Id.eq(id))
            .into_model::<PageSlug>()
            .one(db)
            .await?;
        let slug = res
            .ok_or_else(|| anyhow::Error::msg("Couldn't find data"))?
            .slug;
        let mut conn = ctx
            .data::<RedisBb8ConnectionPool>()
            .expect("Couldn't get redis pool")
            .get()
            .await?;
        let val: Option<String> = conn.get(&slug).await?;
        if let Some(data) = val {
            anyhow::Ok(serde_json::from_str(&data)?)
        } else {
            // find data from postgres
            let res = page::Entity::find_by_id(id)
                .one(db)
                .await?
                .ok_or_else(|| anyhow::Error::msg("Couldn't find data"))?;
            let strified_res = serde_json::to_string(&res)?;
            conn.set(slug, strified_res).await?;
            anyhow::Ok(res)
        }
    }
}

#[derive(Default)]
struct ParsedQueryRoot;

#[Object]
impl ParsedQueryRoot {
    async fn parse(&self, input: String) -> anyhow::Result<i32> {
        let data: i32 = input.parse()?;
        anyhow::Ok(data)
    }
}

#[derive(MergedObject, Default)]
struct QueryRoot(GenericQueryRoot, ParsedQueryRoot);

type TestingSchema = Schema<QueryRoot, EmptyMutation, EmptySubscription>;

async fn graphql_handler(
    schema: Extension<TestingSchema>,
    req: axum::Json<Request>,
) -> axum::Json<Response> {
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

async fn get_from_redis_pool(pool: Extension<RedisBb8ConnectionPool>) -> String {
    let mut conn = pool.get().await.unwrap();
    let val: String = conn.get("Testing Key").await.unwrap();
    val
}

async fn get_from_postgres(db: Extension<Arc<Mutex<DatabaseConnection>>>) -> String {
    let num = rand::thread_rng().gen_range(1..100000);
    let conn = db.lock().await;
    let data = entities::prelude::Page::find_by_id(num)
        .one(&*conn)
        .await
        .expect("Data not found!");
    format!("{}", data.unwrap().id)
}

async fn _get_from_postgres(
    db: Extension<Arc<Mutex<DatabaseConnection>>>,
) -> anyhow::Result<Option<i32>> {
    let num = rand::thread_rng().gen_range(1..100000);
    let conn = db.lock().await;
    let data = entities::prelude::Page::find_by_id(num).one(&*conn).await?;
    if let Some(model) = data {
        return anyhow::Ok(Some(model.id));
    }
    anyhow::Ok(None)
}

#[pyo3_asyncio::tokio::main(flavor = "multi_thread")]
async fn main() -> PyResult<()> {
    pyo3::prepare_freethreaded_python();

    // postgres
    let db = Database::connect("postgres://saleor:saleor@localhost:5432/saleor_fresh")
        .await
        .unwrap();

    // Redis
    let client = redis::Client::open("redis://:@0.0.0.0:6379/4").unwrap();
    let connection = client.get_async_connection().await.unwrap();
    let manager = RedisConnectionManager::new("redis://:@0.0.0.0:6379/4").unwrap();
    let pool = bb8_redis::bb8::Pool::builder()
        .build(manager)
        .await
        .unwrap();

    // Graphql
    let schema: TestingSchema =
        Schema::build(QueryRoot::default(), EmptyMutation, EmptySubscription)
            .data(db)
            .data(pool.clone())
            .finish();

    // pyo3
    let locals = Python::with_gil(pyo3_asyncio::tokio::get_current_locals)?;

    let app = Router::new()
        .route("/graphql", get(graphql_playground).post(graphql_handler))
        .route(
            "/sleep",
            // To call async python function inside axum's routes, we must pass locals.
            get(move || pyo3_asyncio::tokio::scope(locals.clone(), sleep())),
        )
        .route("/rust_sleep", get(rust_sleep))
        .route("/redis_get", get(get_from_redis))
        .route("/redis_pool_get", get(get_from_redis_pool))
        .route("/get_from_postgres", get(get_from_postgres))
        .layer(Extension(schema))
        .layer(Extension(pool))
        .layer(Extension(Arc::new(Mutex::new(connection))));
    // .layer(Extension(Arc::new(Mutex::new(db))));

    axum::Server::bind(&"0.0.0.0:3000".parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();
    PyResult::Ok(())
}
