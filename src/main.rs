#![feature(custom_derive, custom_attribute, plugin)]
#![plugin(diesel_codegen, dotenv_macros)]

#[macro_use] extern crate diesel;
extern crate dotenv;
extern crate iron;
extern crate answeredthis;
extern crate router;
extern crate persistent;
extern crate mustache;
extern crate rustc_serialize;
extern crate params;
#[macro_use] extern crate maplit;
extern crate pulldown_cmark;


use iron::prelude::*;
use iron::status;
use router::Router;
use persistent::Write;
use iron::typemap::Key;
use iron::modifiers::Header;
use iron::modifiers::RedirectRaw;

use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;
use dotenv::dotenv;
use std::env;
use std::io::Cursor;
use iron::headers::ContentType;

use answeredthis::models::*;
use answeredthis::{create_post, update_post};

pub fn establish_connection() -> SqliteConnection {
    dotenv().ok();

    let database_url = env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set");
    SqliteConnection::establish(&database_url)
        .expect(&format!("Error connecting to {}", database_url))
}

#[derive(RustcEncodable)]
struct Answer {
id: String,
    title: String,
    asof: String,
    content: String,
}

impl Answer {
    fn new(post: &Post) -> Self {
        let mut opts = pulldown_cmark::Options::empty();
        opts.insert(pulldown_cmark::OPTION_ENABLE_TABLES);
        opts.insert(pulldown_cmark::OPTION_ENABLE_FOOTNOTES);

        let title = format!("## {}", post.title);
        let mut parser = pulldown_cmark::Parser::new_ext(&title, opts);
        let mut title = String::new();
        pulldown_cmark::html::push_html(&mut title, parser);

        let mut parser = pulldown_cmark::Parser::new_ext(&post.content, opts);
        let mut content = String::new();
        pulldown_cmark::html::push_html(&mut content, parser);

        Answer {
            id: post.id.to_string(),
            title: title,
            asof: post.asof.clone(),
            content: content,
        }
    }
}


fn index_handler(req: &mut Request) -> IronResult<Response> {
    use answeredthis::schema::posts::dsl::*;

    let mutex = req.get::<Write<DbConn>>().unwrap();
    let mut conn = mutex.lock().unwrap();

    let results = posts
        //.limit(5)
        .order(id.desc())
        .load::<Post>(&*conn)
        .expect("Error loading posts");

    let answers = hashmap!{
        "answers" => results.iter().map(|x| Answer::new(x)).collect::<Vec<_>>()
    };

    // First the string needs to be compiled.
    let template = mustache::compile_str(include_str!("home.html"));

    let mut out = Cursor::new(Vec::new());
    template.render(&mut out, &answers).unwrap();

    //let ref query = req.extensions.get::<Router>().unwrap().find("query").unwrap_or("/");
    Ok(Response::with((status::Ok, out.into_inner(), Header(ContentType::html()))))
}

fn new_handler_get(req: &mut Request) -> IronResult<Response> {
    // First the string needs to be compiled.
    let template = mustache::compile_str(include_str!("new_get.html"));

    let mut out = Cursor::new(Vec::new());
    template.render(&mut out, &hashmap!{
        "title" => "".to_string(),
        "content" => "".to_string(),
    }).unwrap();

    Ok(Response::with((status::Ok, out.into_inner(), Header(ContentType::html()))))
}

fn new_handler_post(req: &mut Request) -> IronResult<Response> {
    let mutex = req.get::<Write<DbConn>>().unwrap();
    let mut conn = mutex.lock().unwrap();

    use params::{Params, Value};

    let mut map = req.get_ref::<Params>().unwrap().to_strict_map::<String>().unwrap();
    let title = map.remove("title").unwrap_or("".to_string());
    let content = map.remove("content").unwrap_or("".to_string());

    println!("title {:?}", title);
    println!("content {:?}", content);
    create_post(&conn, &title, &content);

    //let mut out = Cursor::new(Vec::new());
    //template.render(&mut out, &0).unwrap();

    Ok(Response::with((status::Found, RedirectRaw("/".to_string()))))
    //Ok(Response::with((status::Ok, out.into_inner(), Header(ContentType::html()))))
}

fn edit_handler_get(req: &mut Request) -> IronResult<Response> {
    // First the string needs to be compiled.
    let template = mustache::compile_str(include_str!("edit.html"));

    use params::{Params, Value};

    let mut map = req.get_ref::<Params>().unwrap().to_strict_map::<String>().unwrap();

    let id = map.remove("id").unwrap();
    let num_id = id.parse::<i32>().unwrap();

    println!("what {:?}", num_id);

    let mutex = req.get::<Write<DbConn>>().unwrap();
    let mut conn = mutex.lock().unwrap();

    let mut results = {
        use answeredthis::schema::posts::dsl::*;

        posts
        .filter(id.eq(num_id))
        .limit(1)
        .load::<Post>(&*conn)
        .expect("Error loading posts")
    };

    let post = results.pop().unwrap();

    let mut out = Cursor::new(Vec::new());
    template.render(&mut out, &hashmap!{
        "title" => post.title.clone(),
        "content" => post.content.clone(),
        "post_id" => num_id.to_string(),
    }).unwrap();

    Ok(Response::with((status::Ok, out.into_inner(), Header(ContentType::html()))))
}


fn edit_handler_post(req: &mut Request) -> IronResult<Response> {
    let mutex = req.get::<Write<DbConn>>().unwrap();
    let mut conn = mutex.lock().unwrap();

    use params::{Params, Value};

    let mut map = req.get_ref::<Params>().unwrap().to_strict_map::<String>().unwrap();
    let title = map.remove("title").unwrap_or("".to_string());
    let content = map.remove("content").unwrap_or("".to_string());
    let id = map.remove("id").unwrap();

    println!("id {:?}", id);
    println!("title {:?}", title);
    println!("content {:?}", content);
    update_post(&conn, id.parse::<i32>().unwrap(), &title, &content);

    //let mut out = Cursor::new(Vec::new());
    //template.render(&mut out, &0).unwrap();

    Ok(Response::with((status::Found, RedirectRaw("/".to_string()))))
    //Ok(Response::with((status::Ok, out.into_inner(), Header(ContentType::html()))))
}

#[derive(Copy, Clone)]
pub struct DbConn;
impl Key for DbConn { type Value = SqliteConnection; }

fn main() {
    dotenv().ok();
    let port = if env::var("APP_PRODUCTION").is_ok() {
        80
    } else {
        8000
    };

    let db_conn = establish_connection();

    let mut router = Router::new();
    router.get("/", index_handler, "index");
    router.get("/new", new_handler_get, "new_handler_get");
    router.post("/new", new_handler_post, "new_handler_post");
    router.get("/edit", edit_handler_get, "edit_handler_get");
    router.post("/edit", edit_handler_post, "edit_handler_post");

    let mut chain = Chain::new(router);
    chain.link(Write::<DbConn>::both(db_conn));

    let _server = Iron::new(chain)
        .http(&format!("0.0.0.0:{}", port)[..]).unwrap();
    println!("http://0.0.0.0:{}/", port);
}
