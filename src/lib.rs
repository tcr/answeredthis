#![feature(custom_derive, custom_attribute, plugin)]
#![plugin(diesel_codegen, dotenv_macros)]

#[macro_use] extern crate diesel;
extern crate dotenv;
extern crate iron;

pub mod schema;
pub mod models;

use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;
use models::{Post, NewPost};

pub fn create_post<'a>(conn: &SqliteConnection, title: &'a str, content: &'a str) -> usize {
    use schema::posts;

    let new_post = NewPost {
        title: title,
        asof: "October 2016",
        content: content,
    };

    diesel::insert(&new_post).into(posts::table)
        .execute(conn)
        .expect("Error saving new post")
}

pub fn update_post<'a>(conn: &SqliteConnection, new_id: i32, new_title: &'a str, new_content: &'a str) {
    use schema::posts::dsl::*;

    let post = diesel::update(posts.find(new_id))
       .set((title.eq(new_title), (content.eq(new_content))))
       .execute(conn)
       .expect(&format!("Unable to find post {}", new_id));
}
