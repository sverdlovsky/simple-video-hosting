use anyhow::{Context, Result};
use axum::{
    Extension, Router,
    body::Body,
    extract::{Path, Query},
    http::{HeaderValue, Method, StatusCode, header},
    response::{IntoResponse, Json, Response},
    routing::{get, post},
    serve,
};
use axum_extra::extract::CookieJar;
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, errors::ErrorKind};
use serde::Deserialize;
use sqlx::postgres::PgPoolOptions;
use std::{
    net::{
        SocketAddr,
        TcpListener as StdTcpListener,
    },
    time::Duration,
    sync::Arc,
    env,
};
use tokio::net::TcpListener;
use tower_http::cors::CorsLayer;
use socket2::{Domain, Socket, Type};

pub enum AuthError {
    MissingToken,
    ExpiredToken,
    InvalidToken,
}

#[derive(Debug, Deserialize)]
pub struct Claims {
    pub sub: String,
    //pub exp: usize,
}

pub struct Auth {
    decoding_key: DecodingKey,
}

impl Auth {
    pub fn new() -> anyhow::Result<Self> {
        let jwt_secret =
            env::var("JWT_SECRET").context("Environment variable JWT_SECRET not set!")?;

        Ok(Self {
            decoding_key: DecodingKey::from_secret(jwt_secret.as_bytes()),
        })
    }

    pub fn validate(&self, jar: &CookieJar) -> Result<String, AuthError> {
        let token = match jar.get("access_token") {
            Some(c) => c.value().to_string(),
            None => return Err(AuthError::MissingToken),
        };

        let mut validation = Validation::new(Algorithm::HS256);
        validation.validate_exp = true;

        let token_data = match decode::<Claims>(&token, &self.decoding_key, &validation) {
            Ok(data) => data,
            Err(err) => match *err.kind() {
                ErrorKind::ExpiredSignature => return Err(AuthError::ExpiredToken),
                _ => return Err(AuthError::InvalidToken),
            },
        };

        Ok(token_data.claims.sub)
    }
}

#[derive(Clone)]
struct AppState {
    db: sqlx::Pool<sqlx::Postgres>,
    auth: Arc<Auth>,
}

#[derive(Deserialize)]
struct QueryParams {
    limit: Option<i16>,
    search: Option<String>,
    kind: Option<String>,
    tag: Option<i32>,
    user: Option<i32>,
    app: Option<i32>,
    random: Option<bool>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let domain = env::var("DOMAIN").context("Environment variable DOMAIN is not set!")?;
    let dsn = env::var("DATABASE_URL").context("Environment variable DATABASE_URL is not set!")?;
    let pool = PgPoolOptions::new()
        .max_connections(num_cpus::get() as u32 * 2)
        .idle_timeout(Duration::from_secs(300))
        .connect(dsn.as_str())
        .await
        .context("Failed to connect to Postgres")?;

    let state = AppState {
        db: pool,
        auth: Arc::new(Auth::new()?),
    };

    let cors = CorsLayer::new()
        .allow_origin([
            format!("https://{}", domain)
                .parse::<HeaderValue>()
                .unwrap(),
            format!("https://media.{}", domain)
                .parse::<HeaderValue>()
                .unwrap(),
        ])
        .allow_credentials(true)
        .allow_methods([Method::GET, Method::POST])
        .allow_headers([header::CONTENT_TYPE]);

    let app = Router::new()
        .route("/health", get(health))
        .route("/ready", get(ready))
        .route("/videos", get(videos))
        .route("/video/get/{filename}", get(get_video))
        .layer(Extension(Arc::new(state)))
        .layer(cors);

    let ipv4 = env::var("LISTEN_IPV4")
        .map(|v| v.eq_ignore_ascii_case("true"))
        .unwrap_or(true);

    let ipv4_listener = if ipv4 {
        let ipv4_addr: SocketAddr = format!(
            "{}:{}",
            env::var("LISTEN_IPV4_ADDR").unwrap_or_else(|_| "0.0.0.0".into()),
            env::var("LISTEN_IPV4_PORT").unwrap_or_else(|_| "8080".into()),
        )
        .parse()
        .context("Invalid IPv4 listen address")?;

        let socket = Socket::new(Domain::IPV4, Type::STREAM, None)?;

        socket.set_reuse_address(true)?;
        socket.bind(&ipv4_addr.into())?;
        socket.listen(1024)?;

        let std_listener: StdTcpListener = socket.into();
        std_listener.set_nonblocking(true)?;

        Some(TcpListener::from_std(std_listener)?)
    } else {
        None
    };

    let ipv6 = env::var("LISTEN_IPV6")
        .map(|v| v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);

    let ipv6_listener = if ipv6 {
        let ipv6_addr: SocketAddr = format!(
            "[{}]:{}",
            env::var("LISTEN_IPV6_ADDR").unwrap_or_else(|_| "::".into()),
            env::var("LISTEN_IPV6_PORT").unwrap_or_else(|_| "8080".into()),
        )
        .parse()
        .context("Invalid IPv6 listen address")?;

        let socket = Socket::new(Domain::IPV6, Type::STREAM, None)?;

        socket.set_only_v6(true)?;
        socket.set_reuse_address(true)?;
        socket.bind(&ipv6_addr.into())?;
        socket.listen(1024)?;

        let std_listener: StdTcpListener = socket.into();
        std_listener.set_nonblocking(true)?;

        Some(TcpListener::from_std(std_listener)?)
    } else {
        None
    };

    match (ipv4_listener, ipv6_listener) {
        (Some(ipv4), Some(ipv6)) => {
            tokio::try_join!(
                serve(ipv4, app.clone()),
                serve(ipv6, app),
            )?;
        }
        (Some(ipv4), None) => {
            serve(ipv4, app).await?;
        }
        (None, Some(ipv6)) => {
            serve(ipv6, app).await?;
        }
        (None, None) => {
            anyhow::bail!("Both LISTEN_IPV4 and LISTEN_IPV6 are disabled");
        }
    }

    Ok(())
}

async fn health() -> StatusCode {
    StatusCode::OK
}

async fn ready() -> StatusCode {
    StatusCode::OK
}

async fn videos(
    jar: CookieJar,
    Extension(state): Extension<Arc<AppState>>,
    Query(params): Query<QueryParams>,
) -> impl IntoResponse {
    let email = match state.auth.validate(&jar) {
        Ok(email) => email,
        Err(_) => {
            return (StatusCode::OK, Json(Vec::<serde_json::Value>::new())).into_response();
        }
    };

    let row: (serde_json::Value,) =
        match sqlx::query_as("SELECT get_user_videos($1, $2, $3, $4, $5, $6, $7, $8);")
            .bind(&email)
            .bind(&params.limit)
            .bind(&params.search)
            .bind(&params.kind)
            .bind(&params.tag)
            .bind(&params.user)
            .bind(&params.app)
            .bind(&params.random)
            .fetch_one(&state.db)
            .await
        {
            Ok(r) => r,
            Err(e) => {
                eprintln!("DB error: {}", e);
                return (StatusCode::INTERNAL_SERVER_ERROR, "db error").into_response();
            }
        };

    (StatusCode::OK, Json(row.0)).into_response()
}

async fn get_video(
    jar: CookieJar,
    Extension(state): Extension<Arc<AppState>>,
    Path(filename): Path<String>,
) -> impl IntoResponse {
    let email = match state.auth.validate(&jar) {
        Ok(email) => email,
        Err(_) => {
            return (StatusCode::UNAUTHORIZED).into_response();
        }
    };

    let uuid_str = filename.strip_suffix(".mp4").unwrap_or(&filename);

    let has_access: (bool,) = match sqlx::query_as(
        "SELECT EXISTS (
            SELECT 1
            FROM Video_User_Links up
            JOIN Users u ON u.id = up.uid
            WHERE u.email = $1 AND up.vid = $2::uuid
        )",
    )
    .bind(&email)
    .bind(uuid_str)
    .fetch_one(&state.db)
    .await
    {
        Ok(r) => r,
        Err(e) => {
            eprintln!("DB error: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "DB error").into_response();
        }
    };

    if !has_access.0 {
        return (StatusCode::FORBIDDEN, "Access denied").into_response();
    }

    let internal_path = format!("/videos/{}", filename);

    let mut response = Response::new(Body::empty());
    response
        .headers_mut()
        .insert("X-Accel-Redirect", internal_path.parse().unwrap());
    response
        .headers_mut()
        .insert("Content-Type", "video/mp4".parse().unwrap());

    response
}
