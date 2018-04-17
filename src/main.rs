#![feature(plugin, decl_macro)]
#![plugin(rocket_codegen)]

#[macro_use] extern crate rocket;
#[macro_use] extern crate rocket_codegen;
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
#[macro_use]
extern crate serde_json;
extern crate cookie;
extern crate uuid;
extern crate syntect;
extern crate htmlescape;
extern crate futures;
extern crate hyper;
#[macro_use]
extern crate rayon;
extern crate tokio_core;
extern crate hyper_tls;
#[macro_use]
extern crate dotenv_codegen;
#[macro_use]
extern crate lazy_static;
#[macro_use] extern crate cached;


use rocket::State;
use rocket::response::content;

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
use rayon::prelude::*;
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
use cached::UnboundCache;

cached_key!{
    FIB: UnboundCache<String, String> = UnboundCache::new();
    Key = { format!("{}-{}", style, code) };
    fn format_style_html(style: &str, code: &str) -> String = {
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
}

#[derive(RustcEncodable)]
pub struct Answer {
    pub id: String,
    pub title: String,
    pub title_html: String,
    pub asof: String,
    pub content: String,
    pub content_html: String,
    pub answered: bool,
}

impl Answer {
    pub fn new(post: &Post) -> Self {
        lazy_static! {
            static ref RE_LINKS: Regex = Regex::new(r"(?P<l>^|[^\(<])(?P<u>https?://[^\s>\]]+)").unwrap();
            static ref RE_CODE: Regex = Regex::new(r#"<code class="language-(?P<t>[^"]+)">(?P<u>[\s\S]*?)</code>"#).unwrap();
        }

        let mut opts = pulldown_cmark::Options::empty();
        opts.insert(pulldown_cmark::OPTION_ENABLE_TABLES);
        opts.insert(pulldown_cmark::OPTION_ENABLE_FOOTNOTES);

        let title_md = format!("## {}", post.title);
        let parser = pulldown_cmark::Parser::new_ext(&title_md, opts);
        let mut title_html = String::new();
        pulldown_cmark::html::push_html(&mut title_html, parser);

        let content_md = RE_LINKS.replace_all(&post.content, "$l<$u>");
        let parser = pulldown_cmark::Parser::new_ext(&content_md, opts);
        let mut content_html = String::new();
        pulldown_cmark::html::push_html(&mut content_html, parser);

        let content_html = RE_CODE.replace_all(&content_html, |cap: &Captures| {
            format!("<code>{}</code>", format_style_html(cap.name("t").unwrap(), &htmlescape::decode_html(cap.name("u").unwrap()).unwrap()))
        });

        Answer {
            id: post.id.to_string(),
            title: post.title.clone(),
            title_html,
            asof: post.asof.clone(),
            content: post.content.clone(),
            content_html,
            answered: post.published,
        }
    }

    pub fn to_api(&self) -> serde_json::Value {
        json!({
            "asof": self.asof,
            "id": self.id,
            "title": self.title,
            "title_html": self.title_html,
            "content": self.content,
            "content_html": self.content_html,
            "answered": self.answered,
        })
    }
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

fn api_answers(req: &mut Request) -> IronResult<Response> {
    use answeredthis::schema::posts::dsl::*;

    let mutex = req.get::<Write<DbConn>>().unwrap();
    let conn = mutex.lock().unwrap();

    let results = posts
        .filter(published.eq(true))
        .order(id.desc())
        .load::<Post>(&*conn)
        .expect("Error loading posts");

    let json = json!({
        "answers": results.into_iter().map(|x| {
            Answer::new(&x).to_api()
        }).collect::<Vec<_>>(),
        "logged_in": require_login(req),
    });

    Ok(Response::with((
        status::Ok,
        serde_json::to_string(&json).unwrap(),
        Header(ContentType::json()),
    )))
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

pub fn establish_connection() -> SqliteConnection {
    let database_url = env::var("DATABASE_URL").unwrap();
    SqliteConnection::establish(&database_url)
        .expect(&format!("Error connecting to {}", database_url))
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

fn login_get(req: &mut Request) -> IronResult<Response> {
    if !require_login(req) {
        return Ok(github_redirect());
    }
    Ok(Response::with((status::Found, RedirectRaw("/".to_string()))))
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

fn main2() {
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
    router.get("/login", login_get, "login");
    router.get("/oauth/callback", oauth_callback, "oauth_callback");
    router.post("/api/new", new_handler_post, "new_handler_post");
    router.post("/api/edit", edit_handler_post, "edit_handler_post");
    router.get("/api/answers/", api_answers, "answers");

    let mut chain = Chain::new(router);
    chain.link(Write::<GlobState>::both(session_state));
    chain.link(Write::<DbConn>::both(db_conn));
    mount.mount("/", chain);
    mount.mount("/favicon.ico", Static::new(Path::new("static/favicon.ico")));
    mount.mount("/favicon.png", Static::new(Path::new("static/favicon.png")));

    let _server = Iron::new(mount)
        .http(&format!("0.0.0.0:{}", port)[..]).unwrap();
    println!("http://localhost:{}/", port);
}


#[get("/")]
fn index(state: State<BetterSessionState>) -> content::Html<String> {
    let conn = state.db.lock().unwrap();

    let results = {
        use answeredthis::schema::posts::dsl::*;
        
        posts
            //.limit(5)
            .order(id.desc())
            .load::<Post>(&*conn)
            .expect("Error loading posts")
    };

    let data = MapBuilder::new()
        .insert("answers", &results.iter().map(|x| Answer::new(x)).collect::<Vec<_>>()).unwrap()
        
        
        //TODODODODDODO
        // .insert_bool("logged_in", require_login(req))
        //TODODODODDODO

        
        .build();

    // First the string needs to be compiled.
    let template = mustache::compile_str(include_str!("../views/home.html"));

    let mut out = Cursor::new(Vec::new());
    template.render_data(&mut out, &data);

    //let ref query = req.extensions.get::<Router>().unwrap().find("query").unwrap_or("/");
    content::Html(String::from_utf8_lossy(&out.into_inner()).to_string())
}

use std::sync::Arc;
use std::sync::Mutex;

// #[derive(PartialEq, Eq, Hash, Clone, Debug)]
struct BetterSessionState {
    db: Arc<Mutex<SqliteConnection>>,
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

    rocket::ignite()
        .manage(BetterSessionState { db: Arc::new(Mutex::new(db_conn)), })
        .mount("/", routes![index])
        .launch();
}