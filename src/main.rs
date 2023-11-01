use axum::{
    async_trait,
    extract::FromRequestParts,
    http::{request::Parts, StatusCode},
    response::IntoResponse,
    routing::get,
    Router, Server,
};
use maud::{html, Markup, DOCTYPE};

#[tokio::main]
async fn main() {
    let addr: std::net::SocketAddr = "127.0.0.1:9007".parse().expect("addr not parsed");
    Server::bind(&addr)
        .serve(routes().into_make_service())
        .await
        .unwrap();
}

enum Route {
    Home,
    File,
}

impl From<Route> for &'static str {
    fn from(value: Route) -> Self {
        match value {
            Route::Home => "/",
            Route::File => "/pub/*file",
        }
    }
}

fn routes() -> Router {
    let handlers = Router::new().route(Route::Home.into(), get(home));
    let assets = Router::new().route(Route::File.into(), get(files));

    Router::new()
        .nest("", handlers)
        .nest("", assets)
        .fallback(not_found)
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
    InternalServer,
}

type Res<T> = Result<T, Error>;
type Html = Res<Markup>;

struct Context {}

impl Context {
    fn render(&self, markup: Markup) -> Html {
        Ok(html! {
            (DOCTYPE)
            html lang="en" {
                head {
                    title { "links" }
                    meta name="viewport" content="width=device-width, initial-scale=1";
                    meta charset="UTF-8";
                    link rel="stylesheet" href="./pub/tailwind.css";
                }
                body class="bg-white dark:bg-gray-950 dark:text-white" {
                    (markup)
                }
            }
        })
    }
}

impl IntoResponse for Error {
    fn into_response(self) -> axum::response::Response {
        match self {
            Error::NotFound => (StatusCode::NOT_FOUND, "not found").into_response(),
            Error::InternalServer => {
                (StatusCode::INTERNAL_SERVER_ERROR, "internal server error").into_response()
            }
        }
    }
}

#[async_trait]
impl<S> FromRequestParts<S> for Context {
    type Rejection = Error;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        Ok(Context {})
    }
}

async fn home(cx: Context) -> Html {
    let links: Vec<Link> = vec![];
    cx.render(html! {
        div class="max-w-lg mx-auto w-full lg:px-0 px-3 h-screen flex flex-col gap-3" {
            h1 class="text-2xl lg:text-4xl text-center" { "links" }
            div class="flex flex-col w-full gap-3" {
                (text_input("url"))
                (button("Add link"))
                div class="w-full flex flex-col gap-4" {
                    @for link in &links {
                        (link_row(link))
                    }
                }
            }
        }
    })
}

fn spacer() -> Markup {
    html! {
        div class="flex flex-1" {}
    }
}

fn text_input(name: &str) -> Markup {
    html! {
        input autofocus type="text" class="p-2 py-3 text-xl bg-white dark:bg-gray-600 rounded-md outline-none" name=(name) tabindex="1";
    }
}

fn button(name: &str) -> Markup {
    html! {
        button type="submit" class="px-2 py-4 bg-orange-500 rounded-md hover:bg-orange-400" {
            (name)
        }
    }
}

#[derive(Debug)]
struct Link {
    url: String,
}

fn link_row(link: &Link) -> Markup {
    html! {
        div class="" {
            (link.url)
        }
    }
}
