use async_graphql::SimpleObject;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, SimpleObject, DeriveEntityModel)]
#[sea_orm(table_name = "page_with_requirements_mv")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub slug: String,
    pub version: i32,
    pub publication_date: Option<DateTimeWithTimeZone>,
    pub group_name: String,
    pub requirement_type: String,
    pub matches_all: bool,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
