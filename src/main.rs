use std::sync::OnceLock;

use axum::{
    async_trait,
    extract::FromRequestParts,
    http::{request::Parts, StatusCode},
    response::{IntoResponse, Redirect},
    routing::get,
    Json, Router, Server,
};
use maud::{html, Markup, DOCTYPE};
use rizz::{desc, Connection, Database, Real, Table, Text};
use serde::{Deserialize, Serialize};

#[tokio::main]
async fn main() -> Res<()> {
    let db = database().await?;
    migrate(&db).await?;
    let addr: std::net::SocketAddr = "127.0.0.1:9007".parse().expect("addr not parsed");
    println!("Listening on localhost:9007");
    Server::bind(&addr)
        .serve(routes().into_make_service())
        .await
        .unwrap();

    Ok(())
}

#[derive(Clone)]
enum Route {
    Home,
    File,
}

impl std::fmt::Display for Route {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let x: &str = self.to_owned().into();
        f.write_str(x)
    }
}

impl From<Route> for &'static str {
    fn from(value: Route) -> Self {
        match &value {
            Route::Home => "/",
            Route::File => "/pub/*file",
        }
    }
}

fn routes() -> Router {
    let handlers = Router::new().route(Route::Home.into(), get(home).post(add_link));
    let assets = Router::new().route(Route::File.into(), get(files));

    Router::new()
        .nest("", handlers)
        .nest("", assets)
        .fallback(not_found)
}

struct HomeComponent {
    error: Option<&'static str>,
    links: Vec<Link>,
}

impl Component for HomeComponent {
    fn html(&self) -> Markup {
        html! {
            form class="flex flex-col w-full gap-3" action=(Route::Home) method="post" {
                (text_input("url"))
                (button("Add link"))
            }
            @if let Some(err) = &self.error {
                (err)
            }
            div class="w-full flex flex-col gap-4 divide-y dark:divide-gray-700 divide-gray-200"  {
                @for link in &self.links {
                    a class="text-2xl text-sky-500 underline hover:text-sky-300" href=(link.url) {
                        (link.url)
                    }
                }
            }
        }
    }
}

async fn home(cx: Context) -> Html {
    let error = None;
    let links = cx.links().await?;
    let home = HomeComponent { error, links };

    cx.render(home)
}

#[derive(Deserialize, Serialize)]
struct LinkParams {
    url: String,
}

async fn add_link(cx: Context, Json(params): Json<LinkParams>) -> Res<impl IntoResponse> {
    if !params.url.starts_with("https://") {
        let links = cx.links().await?;
        let error = Some("Url needs to start with https://".into());
        let home = HomeComponent { error, links };
        return Ok(cx.render(home).into_response());
    }
    let Context { db, links } = cx;
    let _rows_affected = db
        .insert_into(links)
        .values(Link {
            url: params.url,
            created_at: now(),
            id: nanoid::nanoid!(),
        })?
        .rows_affected()
        .await?;
    Ok(Redirect::to(Route::Home.into()).into_response())
}

async fn not_found() -> impl IntoResponse {
    Error::NotFound
}

async fn files(uri: axum::http::Uri) -> impl IntoResponse {
    let mut path = uri.path().trim_start_matches('/').to_string();
    if path.starts_with("pub/") {
        path = path.replace("pub/", "");
    }
    StaticFile(path)
}

#[derive(rust_embed::RustEmbed)]
#[folder = "pub"]
pub struct Files;

pub struct StaticFile<T>(pub T);

impl<T> StaticFile<T>
where
    T: Into<String>,
{
    fn maybe_response(self) -> Res<axum::response::Response> {
        let path = self.0.into();
        let asset = Files::get(path.as_str()).ok_or(Error::NotFound)?;
        let body = axum::body::boxed(axum::body::Full::from(asset.data));
        let mime = mime_guess::from_path(path).first_or_octet_stream();
        let response = axum::response::Response::builder()
            .header(axum::http::header::CONTENT_TYPE, mime.as_ref())
            .header(axum::http::header::CACHE_CONTROL, "public, max-age=604800")
            .body(body)
            .map_err(|_| Error::NotFound)?;
        Ok(response)
    }
}

impl<T> IntoResponse for StaticFile<T>
where
    T: Into<String>,
{
    fn into_response(self) -> axum::response::Response {
        self.maybe_response()
            .unwrap_or(Error::NotFound.into_response())
    }
}

#[derive(Debug)]
enum Error {
    NotFound,
    Database(String),
}

type Res<T> = Result<T, Error>;
type Html = Res<Markup>;

#[derive(Clone)]
struct Context {
    db: Database,
    links: Links,
}

trait Component {
    fn html(&self) -> Markup;
}

impl Context {
    fn render(&self, component: impl Component) -> Html {
        Ok(html! {
            (DOCTYPE)
            html lang="en" {
                head {
                    title { "links" }
                    meta name="viewport" content="width=device-width, initial-scale=1";
                    meta charset="UTF-8";
                    link rel="stylesheet" href="./pub/tailwind.css";
                    script src="./pub/htmx.org@1.9.5.js" {}
                    script src="./pub/json-enc.js" {}
                }
                body class="bg-white dark:bg-gray-950 dark:text-white" hx-boost="true" hx-ext="json-enc" {
                    div class="max-w-lg mx-auto w-full lg:px-0 px-3 h-screen flex flex-col gap-3" {
                        h1 class="text-2xl lg:text-4xl text-center" { "links" }
                        (component.html())
                    }
                }
            }
        })
    }

    async fn links(&self) -> Res<Vec<Link>> {
        let Context { db, links } = &self;
        let rows = db
            .select()
            .from(*links)
            .order(vec![(desc(links.created_at))])
            .limit(10)
            .all()
            .await?;
        Ok(rows)
    }
}

impl IntoResponse for Error {
    fn into_response(self) -> axum::response::Response {
        match self {
            Error::NotFound => (StatusCode::NOT_FOUND, "not found").into_response(),
            Error::Database(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, "internal server error").into_response()
            }
        }
    }
}

#[async_trait]
impl<S> FromRequestParts<S> for Context {
    type Rejection = Error;

    async fn from_request_parts(_parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        Ok(Context {
            db: database().await?,
            links: Links::new(),
        })
    }
}

fn text_input(name: &str) -> Markup {
    html! {
        input autofocus type="text" class="p-2 py-3 text-xl bg-gray-100 dark:bg-gray-600 rounded-md outline-none" name=(name) tabindex="0";
    }
}

fn button(name: &str) -> Markup {
    html! {
        button type="submit" class="px-2 py-4 bg-orange-500 rounded-md hover:bg-orange-400" {
            (name)
        }
    }
}

fn now() -> f64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now();

    now.duration_since(UNIX_EPOCH).unwrap().as_secs_f64()
}

impl From<rizz::Error> for Error {
    fn from(value: rizz::Error) -> Self {
        match value {
            rizz::Error::ConnectionClosed => todo!(),
            rizz::Error::Close(_) => todo!(),
            rizz::Error::Database(err) => Error::Database(err),
            rizz::Error::MissingFrom => todo!(),
            rizz::Error::InsertError(_) => todo!(),
            rizz::Error::SqlConversion(_) => todo!(),
            rizz::Error::RowNotFound => Error::NotFound,
        }
    }
}

static DATABASE: OnceLock<Database> = OnceLock::new();

async fn database() -> Res<Database> {
    let database = match DATABASE.get() {
        Some(database) => database.clone(),
        None => {
            let connection = Connection::new("db.sqlite3")
                .create_if_missing(true)
                .journal_mode(rizz::JournalMode::Wal)
                .synchronous(rizz::Synchronous::Normal)
                .foreign_keys(true)
                .open()
                .await?;
            let database = connection.database();
            let _x = DATABASE
                .set(database.clone())
                .expect("failed to set DATABASE");
            DATABASE.get().expect("failed to get DATABASE").clone()
        }
    };
    Ok(database)
}

#[derive(Serialize, Deserialize)]
struct Link {
    id: String,
    url: String,
    created_at: f64,
}

#[allow(unused)]
#[derive(Table, Clone, Copy)]
#[rizz(table = "links")]
struct Links {
    #[rizz(primary_key)]
    id: Text,
    #[rizz(not_null)]
    url: Text,
    #[rizz(not_null)]
    created_at: Real,
}

async fn migrate(db: &Database) -> Res<()> {
    let links = Links::new();
    db.create_table(links)
        .create_unique_index(links, vec![links.url])
        .migrate()
        .await?;
    Ok(())
}
