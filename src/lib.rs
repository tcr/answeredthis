#![feature(custom_derive, custom_attribute, plugin)]

#[macro_use] extern crate diesel;
#[macro_use] extern crate diesel_codegen;
extern crate dotenv;
extern crate iron;
extern crate chrono;
#[macro_use]
extern crate serde_derive;

pub mod schema;
pub mod models;

use chrono::UTC;
use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;
use models::NewPost;

pub fn create_post<'a>(
    conn: &SqliteConnection,
    title: &'a str,
    content: &'a str,
) -> usize {
    use schema::posts;

    let date = UTC::now().date().format("%B %Y").to_string();

    let new_post = NewPost {
        title: title,
        asof: &date,
        content: content,
    };

    diesel::insert(&new_post).into(posts::table)
        .execute(conn)
        .expect("Error saving new post")
}

pub fn update_post<'a>(
    conn: &SqliteConnection,
    new_id: i32,
    new_title: &'a str,
    new_content: &'a str,
) {
    use schema::posts::dsl::*;

    let _ = diesel::update(posts.find(new_id))
       .set((
            title.eq(new_title), 
            (
                content.eq(new_content)),
            )
        )
       .execute(conn)
       .expect(&format!("Unable to find post {}", new_id));
}
