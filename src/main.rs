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
}

impl From<Route> for &'static str {
    fn from(value: Route) -> Self {
        match value {
            Route::Home => "/",
        }
    }
}

fn routes() -> Router {
    Router::new().route(Route::Home.into(), get(home))
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
            html {
                head {}
                body {
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
    cx.render(html! {
        h1 { "hello world!" }
    })
}
