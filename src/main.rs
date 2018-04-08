// #[macro_use] extern crate diesel_codegen;
// #[macro_use]
extern crate diesel;
#[macro_use] extern crate maplit;
extern crate answeredthis;
extern crate iron;
extern crate mount;
extern crate mustache;
extern crate params;
extern crate persistent;
extern crate pulldown_cmark;
extern crate regex;
extern crate dotenv;
extern crate router;
extern crate rustc_serialize;
extern crate staticfile;
extern crate serde_json;
extern crate cookie;
extern crate uuid;
extern crate syntect;
extern crate htmlescape;
extern crate futures;
extern crate hyper;
extern crate tokio_core;
extern crate hyper_tls;
#[macro_use]
extern crate dotenv_codegen;

use answeredthis::{create_post, update_post};
use answeredthis::models::*;
use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;
use dotenv::dotenv;
use iron::headers::ContentType;
use iron::modifiers::{Header};
use cookie::CookieJar;
use iron::modifiers::RedirectRaw;
use iron::prelude::*;
use std::str::FromStr;
use iron::status;
use std::collections::HashMap;
use mount::Mount;
use hyper::{Client, Method, Chunk};
use futures::{Future, Stream};
use tokio_core::reactor::Core;
use mustache::MapBuilder;
// use hyper::header;
use iron::headers;
use persistent::Write;
use regex::{Captures, Regex};
use router::Router;
use staticfile::Static;
use std::env;
use std::io::Cursor;
use std::path::Path;
use iron::typemap;
use uuid::Uuid;
use syntect::easy::HighlightLines;
use syntect::parsing::SyntaxSet;
use syntect::highlighting::ThemeSet;
use syntect::html::{styles_to_coloured_html, IncludeBackground};

fn format_style_html(style: &str, code: &str) -> String {
    // Load these once at the start of your program
    let ps = SyntaxSet::load_defaults_nonewlines();
    let ts = ThemeSet::load_defaults();

    let key = match style {
        "rust" => "Rust",
        "sh" => "Shell Script (Bash)",
        "ruby" => "Ruby",
        "c" => "C",
        _ => return code.to_string(),
    };

    let syntax = ps.find_syntax_by_name(key).expect("Unexpected style");
    let mut h = HighlightLines::new(syntax, &ts.themes["InspiredGitHub"]);
    let regions = h.highlight(code);
    styles_to_coloured_html(&regions[..], IncludeBackground::No)
}

pub fn establish_connection() -> SqliteConnection {
    let database_url = env::var("DATABASE_URL").unwrap();
    SqliteConnection::establish(&database_url)
        .expect(&format!("Error connecting to {}", database_url))
}

#[derive(RustcEncodable)]
struct Answer {
    id: String,
    title: String,
    asof: String,
    content: String,
    answered: bool,
}

impl Answer {
    fn new(post: &Post) -> Self {
        let mut opts = pulldown_cmark::Options::empty();
        opts.insert(pulldown_cmark::OPTION_ENABLE_TABLES);
        opts.insert(pulldown_cmark::OPTION_ENABLE_FOOTNOTES);

        let title = format!("## {}", post.title);
        let parser = pulldown_cmark::Parser::new_ext(&title, opts);
        let mut title = String::new();
        pulldown_cmark::html::push_html(&mut title, parser);

        let re = Regex::new(r"(?P<l>^|[^\(<])(?P<u>https?://[^\s>\]]+)").unwrap();
        let new_content = re.replace_all(&post.content, "$l<$u>");

        let parser = pulldown_cmark::Parser::new_ext(&new_content, opts);
        let mut content = String::new();
        pulldown_cmark::html::push_html(&mut content, parser);

        let code = Regex::new(r#"<code class="language-(?P<t>[^"]+)">(?P<u>[\s\S]*?)</code>"#).unwrap();
        let content = code.replace_all(&content, |cap: &Captures| {
            format!("<code>{}</code>", format_style_html(cap.name("t").unwrap(), &htmlescape::decode_html(cap.name("u").unwrap()).unwrap()))
        });

        Answer {
            id: post.id.to_string(),
            title: title,
            asof: post.asof.clone(),
            content: content,
            answered: post.published,
        }
    }
}


fn index_handler(req: &mut Request) -> IronResult<Response> {
    use answeredthis::schema::posts::dsl::*;

    let mutex = req.get::<Write<DbConn>>().unwrap();
    let conn = mutex.lock().unwrap();

    let results = posts
        //.limit(5)
        .order(id.desc())
        .load::<Post>(&*conn)
        .expect("Error loading posts");

    let data = MapBuilder::new()
        .insert("answers", &results.iter().map(|x| Answer::new(x)).collect::<Vec<_>>()).unwrap()
        .insert_bool("logged_in", require_login(req))
        .build();

    // First the string needs to be compiled.
    let template = mustache::compile_str(include_str!("../views/home.html"));

    let mut out = Cursor::new(Vec::new());
    template.render_data(&mut out, &data);

    //let ref query = req.extensions.get::<Router>().unwrap().find("query").unwrap_or("/");
    Ok(Response::with((status::Ok, out.into_inner(), Header(ContentType::html()))))
}

fn new_handler_get(req: &mut Request) -> IronResult<Response> {
    if !require_login(req) {
        return Ok(github_redirect());
    }

    // First the string needs to be compiled.
    let template = mustache::compile_str(include_str!("../views/new_get.html"));

    let mut out = Cursor::new(Vec::new());
    template.render(&mut out, &hashmap!{
        "title" => "".to_string(),
        "content" => "".to_string(),
    }).unwrap();

    Ok(Response::with((status::Ok, out.into_inner(), Header(ContentType::html()))))
}

fn form_truthy(value: &str) -> bool {
    value != "" && value != "no" && value != "false" && value != "off"
}

fn new_handler_post(req: &mut Request) -> IronResult<Response> {
    if !require_login(req) {
        return Ok(github_redirect());
    }

    let mutex = req.get::<Write<DbConn>>().unwrap();
    let conn = mutex.lock().unwrap();

    use params::Params;

    let mut map = req.get_ref::<Params>().unwrap().to_strict_map::<String>().unwrap();
    let title = map.remove("title").unwrap_or("".to_string());
    let content = map.remove("content").unwrap_or("".to_string());
    let answered = map.remove("answered").unwrap_or("".to_string());

    println!("title {:?}", title);
    println!("content {:?}", content);
    println!("answered {:?}", answered);
    create_post(&conn, &title, &content, form_truthy(&answered));

    //let mut out = Cursor::new(Vec::new());
    //template.render(&mut out, &0).unwrap();

    Ok(Response::with((status::Found, RedirectRaw("/".to_string()))))
    //Ok(Response::with((status::Ok, out.into_inner(), Header(ContentType::html()))))
}

fn edit_handler_get(req: &mut Request) -> IronResult<Response> {
    if !require_login(req) {
        return Ok(github_redirect());
    }

    // First the string needs to be compiled.
    let template = mustache::compile_str(include_str!("../views/edit.html"));

    use params::Params;

    let mut map = req.get_ref::<Params>().unwrap().to_strict_map::<String>().unwrap();

    let id = map.remove("id").unwrap();
    let num_id = id.parse::<i32>().unwrap();

    println!("what {:?}", num_id);

    let mutex = req.get::<Write<DbConn>>().unwrap();
    let conn = mutex.lock().unwrap();

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
    let mut hey = hashmap!{
        "title" => post.title.clone(),
        "content" => post.content.clone(),
        "post_id" => num_id.to_string(),
    };
    if post.published {
        hey.insert("answered", "checked".to_string());
    }
    template.render(&mut out, &hey).unwrap();

    Ok(Response::with((status::Ok, out.into_inner(), Header(ContentType::html()))))
}

fn require_login(req: &mut Request) -> bool {
    let mutex = req.get::<Write<GlobState>>().unwrap();
    let session_state = mutex.lock().unwrap();

    if let Some(cookie) = req.headers.get::<headers::Cookie>() {
        for item in cookie.0.clone() {
            let item = cookie::Cookie::parse(item).unwrap();
            if item.name() == "session" {
                if let Ok(uuid) = Uuid::parse_str(item.value()) {
                    return session_state.get(&uuid).is_some()
                }
            }
        }
    }
    false
}

fn github_redirect() -> Response {
    dotenv().ok();

    Response::with((status::Found, RedirectRaw(
        format!("https://github.com/login/oauth/authorize?client_id={}&redirect_uri={}",
        env::var("OAUTH_KEY").unwrap(),
        env::var("OAUTH_CALLBACK").unwrap(),
    ))))
}


fn edit_handler_post(req: &mut Request) -> IronResult<Response> {
    if !require_login(req) {
        return Ok(github_redirect());
    }

    let mutex = req.get::<Write<DbConn>>().unwrap();
    let conn = mutex.lock().unwrap();

    use params::Params;

    let mut map = req.get_ref::<Params>().unwrap().to_strict_map::<String>().unwrap();
    let title = map.remove("title").unwrap_or("".to_string());
    let content = map.remove("content").unwrap_or("".to_string());
    let id = map.remove("id").unwrap();
    let answered = map.remove("answered").unwrap_or("".to_string());

    println!("id {:?}", id);
    println!("title {:?}", title);
    println!("content {:?}", content);
    println!("answered {:?}", answered);
    update_post(&conn, id.parse::<i32>().unwrap(), &title, &content, form_truthy(&answered));

    //let mut out = Cursor::new(Vec::new());
    //template.render(&mut out, &0).unwrap();

    Ok(Response::with((status::Found, RedirectRaw("/".to_string()))))
    //Ok(Response::with((status::Ok, out.into_inner(), Header(ContentType::html()))))
}

pub fn fetch_post(url: &str) -> serde_json::Result<serde_json::Value> {
    let mut core = Core::new().unwrap();

    let client = ::hyper::Client::configure()
        .connector(::hyper_tls::HttpsConnector::new(4, &core.handle()).unwrap())
        .build(&core.handle());
    let mut req = hyper::Request::new(Method::Post, hyper::Uri::from_str(url).unwrap());
    req.headers_mut().set(::hyper::header::Connection::close());
    req.headers_mut().set(::hyper::header::UserAgent::new("answeredthis".to_string()));
    req.headers_mut().set(::hyper::header::Accept(vec!["application/json".parse().unwrap()]));
    let mut res = client.request(req);
    
    let mut res = core.run(res).unwrap();

    core.run(res.body().concat2().map(move |body: Chunk| {
        serde_json::from_slice::<serde_json::Value>(&body)
    })).unwrap()
}

pub fn fetch_get(url: &str) -> serde_json::Result<serde_json::Value> {
    let mut core = Core::new().unwrap();
    
    let client = ::hyper::Client::configure()
        .connector(::hyper_tls::HttpsConnector::new(4, &core.handle()).unwrap())
        .build(&core.handle());
    let mut req = hyper::Request::new(Method::Get, hyper::Uri::from_str(url).unwrap());
    req.headers_mut().set(::hyper::header::Connection::close());
    req.headers_mut().set(::hyper::header::UserAgent::new("answeredthis".to_string()));
    req.headers_mut().set(::hyper::header::Accept(vec!["application/json".parse().unwrap()]));
    let mut res = client.request(req);
    
    let mut res = core.run(res).unwrap();

    core.run(res.body().concat2().map(move |body: Chunk| {
        serde_json::from_slice::<serde_json::Value>(&body)
    })).unwrap()
}

fn oauth_callback(req: &mut Request) -> IronResult<Response> {
    use params::Params;

    let mutex = req.get::<Write<GlobState>>().unwrap();
    let mut session_state = mutex.lock().unwrap();

    let mut map = req.get_ref::<Params>().unwrap().to_strict_map::<String>().unwrap();
    let code = map.remove("code").unwrap_or("".to_string());

    let json = fetch_post(
        &format!("https://github.com/login/oauth/access_token?client_id={}&client_secret={}&code={}",
            env::var("OAUTH_KEY").unwrap(),
            env::var("OAUTH_SECRET").unwrap(),
            code,
        ),
    );
    let user = fetch_get(
        &format!("https://api.github.com/user?access_token={}",
            json.unwrap().pointer("/access_token").unwrap().as_str().unwrap(),
        ),
    );
    let username = user.unwrap().pointer("/login").unwrap().as_str().unwrap().to_string();

    if username == "tcr" {
        println!("SET THAT COOKIE");

        let uuid = Uuid::new_v4();
        let mut jar = CookieJar::new();
        let cookie = cookie::Cookie::new("session".to_string(), uuid.to_string());
        jar.add(cookie);

        let delta = jar.delta().map(|item| {
            let mut c = item.clone();
            c.set_path("/".to_string());
            c.to_string()
        }).collect::<Vec<_>>();
        let set = headers::SetCookie(delta);

        session_state.insert(uuid, SessionState {
            github_id: username.to_string()
        });

        Ok(Response::with((status::Found, RedirectRaw("/".to_string()), iron::modifiers::Header(set))))
    } else {
        Ok(Response::with((status::Found, RedirectRaw("/".to_string()))))
    }
}

#[derive(Copy, Clone)]
pub struct DbConn;
impl typemap::Key for DbConn { type Value = SqliteConnection; }

#[derive(Copy, Clone)]
struct GlobState;
impl typemap::Key for GlobState { type Value = HashMap<Uuid, SessionState>; }

#[derive(PartialEq, Eq, Hash, Clone, Debug)]
struct SessionState {
    github_id: String,
}

fn main() {
    dotenv().ok();

    let port = if env::var("APP_PRODUCTION").is_ok() {
        80
    } else {
        8000
    };
    println!("port {:?}", port);

    let db_conn = establish_connection();

    let session_state = HashMap::new();

    let mut mount = Mount::new();
    mount.mount("/static", Static::new(Path::new("static/")));

    let mut router = Router::new();
    router.get("/", index_handler, "index");
    router.get("/new", new_handler_get, "new_handler_get");
    router.post("/new", new_handler_post, "new_handler_post");
    router.get("/edit", edit_handler_get, "edit_handler_get");
    router.post("/edit", edit_handler_post, "edit_handler_post");
    router.get("/oauth/callback", oauth_callback, "oauth_callback");

    let mut chain = Chain::new(router);
    chain.link(Write::<GlobState>::both(session_state));
    chain.link(Write::<DbConn>::both(db_conn));
    mount.mount("/", chain);

    let _server = Iron::new(mount)
        .http(&format!("0.0.0.0:{}", port)[..]).unwrap();
    println!("http://localhost:{}/", port);
}
