#![feature(custom_derive, custom_attribute, plugin)]

#[macro_use] extern crate diesel;
#[macro_use] extern crate diesel_codegen;
extern crate dotenv;
extern crate iron;
extern crate chrono;

pub mod schema;
pub mod models;

use chrono::UTC;
use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;
use models::NewPost;

pub fn create_post<'a>(conn: &SqliteConnection, title: &'a str, content: &'a str, answered: bool) -> usize {
    use schema::posts;

    let date = UTC::now().date().format("%B %Y").to_string();

    let new_post = NewPost {
        title: title,
        asof: &date,
        content: content,
        published: answered,
    };

    diesel::insert(&new_post).into(posts::table)
        .execute(conn)
        .expect("Error saving new post")
}

pub fn update_post<'a>(conn: &SqliteConnection, new_id: i32, new_title: &'a str, new_content: &'a str, new_answered: bool) {
    use schema::posts::dsl::*;

    let _ = diesel::update(posts.find(new_id))
       .set((title.eq(new_title), (content.eq(new_content)), (published.eq(new_answered))))
       .execute(conn)
       .expect(&format!("Unable to find post {}", new_id));
}
