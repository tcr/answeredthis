#![feature(plugin, decl_macro, custom_derive)]
#![plugin(rocket_codegen)]

#![allow(warnings)]

#[macro_use] extern crate cached;
#[macro_use] extern crate lazy_static;
#[macro_use] extern crate maplit;
#[macro_use] extern crate rayon;
#[macro_use] extern crate rocket_codegen;
#[macro_use] extern crate rocket;
#[macro_use] extern crate serde_json;
extern crate ammonia;
extern crate answeredthis;
extern crate cookie;
extern crate diesel;
extern crate dotenv;
extern crate futures;
extern crate htmlescape;
extern crate hyper_tls;
extern crate hyper;
extern crate iron;
extern crate mount;
extern crate reqwest;
extern crate mustache;
extern crate opengraph;
extern crate params;
extern crate persistent;
extern crate pulldown_cmark;
extern crate regex;
extern crate router;
extern crate rustc_serialize;
extern crate scraper;
extern crate syntect;
extern crate tokio_core;
extern crate url;
extern crate uuid;


use answeredthis::{create_post, update_post};
use answeredthis::models::*;
use cached::UnboundCache;
use cookie::CookieJar;
use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;
use dotenv::dotenv;
use futures::{Future, Stream};
use hyper::{Client, Method, Chunk};
use mustache::MapBuilder;
use rayon::prelude::*;
use regex::{Captures, Regex};
use reqwest::header::UserAgent;
use rocket::config::{Config, Environment};
use rocket::http::{Cookie, Cookies};
use rocket::request::Form;
use rocket::response::{NamedFile, Redirect};
use rocket::response::content;
use rocket::State;
use std::collections::HashMap;
use std::env;
use std::io::Cursor;
use std::path::Path;
use std::path::PathBuf;
use std::str::FromStr;
use syntect::easy::HighlightLines;
use syntect::highlighting::ThemeSet;
use syntect::html::{styles_to_coloured_html, IncludeBackground};
use syntect::parsing::SyntaxSet;
use tokio_core::reactor::Core;
use uuid::Uuid;
use url::Url;
use scraper::{Selector, Html};

cached_key! {
    HIGHLIGHT_CACHE: UnboundCache<String, String> = UnboundCache::new();
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

#[derive(Clone, Debug)]
pub struct Preview {
    site_name: Option<String>,
    title: String,
    url: Url,
    description: Option<String>,
    image: Option<String>,
}

impl Preview {
    fn to_html(&self) -> String {
        format!(
            r#"<div class="preview"><a class="preview-link" href="{url}"><b class="preview-title">{title}</b> <span class="preview-url">{url}</span> <span class="preview-description">{description}</span></a></div>"#,
            title=htmlescape::encode_minimal(&self.title),
            url=htmlescape::encode_minimal(self.url.as_str()),
            description=self.description.as_ref().map(|x| {
                htmlescape::encode_minimal(x)
            }).unwrap_or("".to_string()),
        ).trim().to_string().replace("\n", " ")
    }
}

cached_key! {
    PREVIEW_CACHE: UnboundCache<String, Preview> = UnboundCache::new();
    Key = { format!("{}", url.to_string()) };
    fn preview(url: &Url) -> Preview = {
        let bail = Preview {
            title: url.to_string(),
            url: url.clone(),
            description: None,
            site_name: None,
            image: None,
        };

        let client = reqwest::Client::new();
        let mut html = match client.get(url.as_str())
            .header(UserAgent::new("facebookexternalhit/1.1"))
            .send() {
            Ok(mut x) => x.text().unwrap(),
            Err(_) => return bail,
        };

        let mut html2 = html.clone();
        let mut cur = ::std::io::Cursor::new(&mut html2);
        let og_summary = match opengraph::extract(&mut cur, Default::default()) {
            Ok(object) => {
                // Check that any data was meaningful
                if object.title.len() > 0 {
                    Some(Preview {
                        site_name: object.site_name.clone(),
                        title: object.title.clone(),
                        url: url.clone(),
                        description: object.description.clone(),
                        image: object.images.get(0).map(|x| x.url.clone()),
                    })
                } else {
                    None
                }
            },
            Err(_) => {
                eprintln!("Could not look up opengraph info for {:?}", url);
                None
            }
        };

        if let Some(og_summary) = og_summary {
            og_summary
        } else {
            let document = Html::parse_document(&html);

            let title =
                document.select(&Selector::parse("title").unwrap())
                .next()
                .map(|title| {
                    title.text().collect::<Vec<_>>().join("")
                })
                .unwrap_or_else(|| url.to_string());

            let description =
                document.select(&Selector::parse("body").unwrap())
                .next()
                .map(|body| {
                    const DESC_LIMIT: usize = 200;
                    let mut description = String::new();
                    for text in body.text() {
                        description.push_str(text.trim());
                        if description.len() > DESC_LIMIT + 10 {
                            description = format!("{}...", &description[0..DESC_LIMIT]);
                            break;
                        }
                    }
                    description
                });

            Preview {
                title,
                url: url.clone(),
                description,
                site_name: None,
                image: None
            }
        }
    }
}

impl Answer {
    pub fn new(post: &Post) -> Self {
        lazy_static! {
            static ref RE_CODE: Regex = Regex::new(r#"<code class="language-(?P<t>[^"]+)">(?P<u>[\s\S]*?)</code>"#).unwrap();
            static ref RE_UNANSWERED: Regex = Regex::new(r#"(?m)^\s*!?UNANSWERED\b(\s*)$"#).unwrap();
            static ref RE_PREVIEW: Regex = Regex::new(r#"(?m)^\s*!PREVIEW\b(.*)$"#).unwrap();
            static ref RE_SOURCES: Regex = Regex::new(r#"(?m)^\s*!SOURCES\s*$"#).unwrap();
        }

        let mut sanitizer = ammonia::Builder::default();
        sanitizer.allowed_classes(hashmap![
            "div" => hashset!["preview", "preview-title", "preview-url", "preview-description", "sources"],
            "a" => hashset!["preview-link"],
            "span" => hashset!["preview", "preview-title", "preview-url", "preview-description"],
        ]);

        let mut opts = pulldown_cmark::Options::empty();
        opts.insert(pulldown_cmark::OPTION_ENABLE_TABLES);
        opts.insert(pulldown_cmark::OPTION_ENABLE_FOOTNOTES);

        // Format title.
        let title_md = format!("## {}", post.title);
        let parser = pulldown_cmark::Parser::new_ext(&title_md, opts);
        let mut title_html = String::new();
        pulldown_cmark::html::push_html(&mut title_html, parser);
        let title_html = sanitizer.clean(&title_html).to_string();

        // Detects "UNANSWERED" directve
        let answered = !RE_UNANSWERED.is_match(&post.content);

        // Format content.
        let content_md = post.content.clone();
        let content_md = RE_UNANSWERED.replace_all(&content_md, "");
        let content_md = RE_PREVIEW.replace_all(&content_md, |cap: &Captures| {
            let raw_string = cap.at(0).unwrap().to_string();
            let url = match Url::from_str(&cap.at(1).unwrap()) {
                Ok(url) => url,
                Err(err) => {
                    eprintln!("Error parsing !PREVIEW: {:?}", err);
                    eprintln!("Parsed: {:?}", cap.at(1));
                    return raw_string;
                }
            };

            preview(&url).to_html()
        });
        let content_md = RE_SOURCES.replace_all(&content_md, "\n\n<div class=\"sources\">Sources:</div>\n\n");

        // Parse content.
        let parser = pulldown_cmark::Parser::new_ext(&content_md, opts);
        let mut content_html = String::new();
        pulldown_cmark::html::push_html(&mut content_html, parser);
        let content_html = sanitizer.clean(&content_html).to_string();

        // Convert code blocks with language codes
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
            answered,
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













#[derive(FromForm, Debug)]
struct ApiEditForm {
    id: String,
    title: String,
    content: String,
}

#[post("/api/edit", data = "<form>")]
fn api_edit(mut cookies: Cookies, form: Form<ApiEditForm>, state: State<BetterSessionState>) -> Redirect {
    if !require_login(&mut cookies) {
        return github_redirect();
    }

    let conn = state.db.lock().unwrap();

    println!("form {:?}", form);
    let form = form.get();
    update_post(&conn, form.id.parse::<i32>().unwrap(), &form.title, &form.content);

    Redirect::to("/")
}

#[derive(FromForm, Debug)]
struct ApiNewForm {
    title: String,
    content: String,
}

#[post("/api/new", data = "<form>")]
fn api_new(mut cookies: Cookies, form: Form<ApiNewForm>, state: State<BetterSessionState>) -> Redirect {
    if !require_login(&mut cookies) {
        return github_redirect();
    }

    let conn = state.db.lock().unwrap();

    println!("form {:?}", form);
    let form = form.get();
    create_post(&conn, &form.title, &form.content);

    Redirect::to("/")
}

fn github_redirect() -> Redirect {
    Redirect::to(
        &format!("https://github.com/login/oauth/authorize?client_id={}&redirect_uri={}",
            env::var("OAUTH_KEY").unwrap(),
            env::var("OAUTH_CALLBACK").unwrap(),
        )
    )
}

#[get("/login")]
fn login(mut cookies: Cookies) -> Redirect {
    if !require_login(&mut cookies) {
        return github_redirect();
    }
    Redirect::to("/")
}

#[derive(FromForm, Debug)]
struct OauthCallbackQuery {
    code: Option<String>,
}

#[get("/oauth/callback?<query>")]
fn oauth_callback(mut cookies: Cookies, query: OauthCallbackQuery) -> Redirect {
    let json = fetch_post(
        &format!("https://github.com/login/oauth/access_token?client_id={}&client_secret={}&code={}",
            env::var("OAUTH_KEY").unwrap(),
            env::var("OAUTH_SECRET").unwrap(),
            query.code.expect("Code not provided"),
        ),
    );
    let user = fetch_get(
        &format!("https://api.github.com/user?access_token={}",
            json.unwrap().pointer("/access_token").unwrap().as_str().unwrap(),
        ),
    );
    let username = user.unwrap().pointer("/login").unwrap().as_str().unwrap().to_string();

    if username == "tcr" {
        cookies.add_private(Cookie::new("session", username));
    }
    
    Redirect::to("/")
}

fn require_login(cookies: &mut Cookies) -> bool {
    cookies.get_private("session").is_some()
}

#[get("/api/answers")]
fn api_answers(state: State<BetterSessionState>, mut cookies: Cookies) -> content::Json<String> {
    let conn = state.db.lock().unwrap();

    let results = {
        use answeredthis::schema::posts::dsl::*;

        posts
            .order(id.desc())
            .load::<Post>(&*conn)
            .expect("Error loading posts")
    };

    let json = json!({
        "answers": results.into_iter().map(|x| {
            Answer::new(&x).to_api()
        }).collect::<Vec<_>>(),
        "logged_in": require_login(&mut cookies),
    });

    content::Json(serde_json::to_string(&json).unwrap())
}

#[get("/")]
fn index(state: State<BetterSessionState>, mut cookies: Cookies) -> content::Html<String> {
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
        .insert_bool("logged_in", require_login(&mut cookies))
        .build();

    // First the string needs to be compiled.
    let template = mustache::compile_str(include_str!("../views/home.html"));

    let mut out = Cursor::new(Vec::new());
    template.render_data(&mut out, &data);

    //let ref query = req.extensions.get::<Router>().unwrap().find("query").unwrap_or("/");
    content::Html(String::from_utf8_lossy(&out.into_inner()).to_string())
}

#[get("/static/<file..>")]
fn files(file: PathBuf) -> Option<NamedFile> {
    NamedFile::open(Path::new("static/").join(file)).ok()
}

#[get("/favicon.ico")]
fn favicon_ico() -> Option<NamedFile> {
    NamedFile::open(Path::new("static/favicon.ico")).ok()
}

#[get("/favicon.png")]
fn favicon_png() -> Option<NamedFile> {
    NamedFile::open(Path::new("static/favicon.png")).ok()
}

use std::sync::Arc;
use std::sync::Mutex;

// #[derive(PartialEq, Eq, Hash, Clone, Debug)]
struct BetterSessionState {
    db: Arc<Mutex<SqliteConnection>>,
}

fn main() {
    dotenv().ok();

    println!("# ANSWERED THIS #");

    let db_conn = establish_connection();

    let config = Config::build(Environment::Staging)
        .address("0.0.0.0")
        .port(8000)
        .finalize().unwrap();

    rocket::custom(config, true)
        .manage(BetterSessionState { db: Arc::new(Mutex::new(db_conn)), })
        .mount("/", routes![
            index,
            login,
            oauth_callback,
            api_answers,
            api_new,
            api_edit,

            files,
            favicon_ico,
            favicon_png,
        ])
        .launch();
}