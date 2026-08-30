use anyhow::{Context, Result};
use axum::{
    Extension, Router,
    body::Body,
    extract::{Path, Query},
    http::{
        HeaderMap,
        HeaderValue,
        Method,
        StatusCode,
        header,
    },
    response::{IntoResponse, Json, Response},
    routing::{get, post},
    serve,
};
use aws_sdk_s3::config::Credentials;
use aws_sdk_s3::presigning::PresigningConfig;
use serde::{Deserialize, json};
use socket2::{Domain, Socket, Type};
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


#[derive(Clone)]
struct AppState {
    db: sqlx::Pool<sqlx::Postgres>,
    s3: aws_sdk_s3::Client,
    s3_bucket: String,
    public_upload: bool,
    download_ttl: Duration,
    upload_ttl: Duration,
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

#[derive(serde::Deserialize)]
struct PostVideoPayload {
    title: String,
    description: Option<String>,
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

    let s3_endpoint = env::var("S3_ENDPOINT").context("Environment variable S3_ENDPOINT is not set!")?;
    let s3_region = env::var("S3_REGION").unwrap_or_else(|_| "us-east-1".to_string());
    let s3_access_key = env::var("S3_ACCESS_KEY").context("Environment variable S3_ACCESS_KEY is not set!")?;
    let s3_secret_key = env::var("S3_SECRET_KEY").context("Environment variable S3_SECRET_KEY is not set!")?;
    let s3_bucket = env::var("S3_PUBLIC_BUCKET").unwrap_or_else(|_| "svh".to_string());

    let public_upload = env::var("PUBLIC_UPLOAD")
        .unwrap_or_else(|_| "false".to_string())
        .parse::<bool>()
        .context("Environment variable PUBLIC_UPLOAD must be true or false")?;

    let download_ttl = env::var("DOWNLOAD_TTL")
        .unwrap_or_else(|_| "120".to_string())
        .parse::<u64>()
        .context("Environment variable DOWNLOAD_TTL must be a number of seconds")?;

    let upload_ttl = env::var("UPLOAD_TTL")
        .unwrap_or_else(|_| "300".to_string())
        .parse::<u64>()
        .context("Environment variable UPLOAD_TTL must be a number of seconds")?;

    let s3_credentials = Credentials::new(
        s3_access_key,
        s3_secret_key,
        None,
        None,
        "static",
    );

    let s3_config = aws_sdk_s3::Config::builder()
        .endpoint_url(s3_endpoint)
        .region(aws_sdk_s3::config::Region::new(s3_region))
        .credentials_provider(s3_credentials)
        .force_path_style(true)
        .behavior_version(aws_sdk_s3::config::BehaviorVersion::latest())
        .build();

    let s3_client = aws_sdk_s3::Client::from_conf(s3_config);

    let state = AppState {
        db: pool,
        s3: s3_client,
        s3_bucket,
        public_upload,
        download_ttl: Duration::from_secs(download_ttl),
        upload_ttl: Duration::from_secs(upload_ttl),
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
        .route("/video", get(videos))
        .route("/video/{video_id}/{quality}", get(get_video))
        .route("/video", post(post_video))
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
    headers: HeaderMap,
    Extension(state): Extension<Arc<AppState>>,
    Query(params): Query<QueryParams>,
) -> impl IntoResponse {
    let email = match headers.get("x-user-email") {
        Some(email) => email.to_str().unwrap().to_owned(),
        None => {
            return (StatusCode::UNAUTHORIZED).into_response();
        }
    };

    let row: (serde_json::Value,) = match sqlx::query_as(
        "SELECT get_user_videos($1, $2, $3, $4, $5, $6, $7, $8);"
    )
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
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                "db error",
            ).into_response();
        }
    };

    (StatusCode::OK, Json(row.0)).into_response()
}

async fn get_video(
    headers: HeaderMap,
    Extension(state): Extension<Arc<AppState>>,
    Path((video_id, quality)): Path<(String, String)>,
) -> impl IntoResponse {
    let email = match headers.get("x-user-email") {
        Some(email) => email.to_str().unwrap().to_owned(),
        None => {
            return (StatusCode::UNAUTHORIZED).into_response();
        }
    };

    if !matches!(quality.as_str(), "orig" | "high" | "low") {
        return (StatusCode::BAD_REQUEST, "Invalid quality").into_response();
    }

    let has_access: (bool,) = match sqlx::query_as(
        "SELECT EXISTS (
            SELECT 1
            FROM Video_User_Links up
            JOIN Users u ON u.id = up.uid
            WHERE u.email = $1 AND up.vid = $2::uuid
        )",
    )
    .bind(&email)
    .bind(&video_id)
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

    let object_key = format!("video/{}/{}.mp4", video_id, quality);

    let presigning_config = match PresigningConfig::expires_in(state.download_ttl) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Presigning config error: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "S3 error").into_response();
        }
    };

    let presigned_request = match state
        .s3
        .get_object()
        .bucket(&state.s3_bucket)
        .key(&object_key)
        .presigned(presigning_config)
        .await
    {
        Ok(req) => req,
        Err(e) => {
            eprintln!("S3 presign error: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "S3 error").into_response();
        }
    };

    let full_uri = presigned_request.uri();
    let public_url = full_uri.replacen("/svh/", "/", 1);

    (StatusCode::OK, Json(json!({ "url": public_url }))).into_response()
}

async fn post_video(
    headers: HeaderMap,
    Extension(state): Extension<Arc<AppState>>,
    Json(payload): Json<PostVideoPayload>,
) -> impl IntoResponse {
    let email = match headers.get("x-user-email") {
        Some(email) => email.to_str().unwrap().to_owned(),
        None => {
            return (StatusCode::UNAUTHORIZED).into_response();
        }
    };

    if !state.public_upload {
        let can_upload: (bool,) = match sqlx::query_as(
            "SELECT upload FROM Users WHERE email = $1",
        )
        .bind(&email)
        .fetch_one(&state.db)
        .await
        {
            Ok(r) => r,
            Err(e) => {
                eprintln!("DB error: {}", e);
                return (StatusCode::INTERNAL_SERVER_ERROR, "DB error").into_response();
            }
        };

        if !can_upload.0 {
            return (StatusCode::FORBIDDEN, "Upload not allowed").into_response();
        }
    }

    let video_id: (i16,) = match sqlx::query_as(
        "INSERT INTO Videos (title, description) VALUES ($1, $2) RETURNING id",
    )
    .bind(&payload.title)
    .bind(&payload.description)
    .fetch_one(&state.db)
    .await
    {
        Ok(r) => r,
        Err(e) => {
            eprintln!("DB error: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "DB error").into_response();
        }
    };

    let object_key = format!("video/{}/orig.mp4", video_id.0);

    let presigning_config = match PresigningConfig::expires_in(state.upload_ttl) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Presigning config error: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "S3 error").into_response();
        }
    };

    let presigned_request = match state
        .s3
        .put_object()
        .bucket(&state.s3_bucket)
        .key(&object_key)
        .content_type("video/mp4")
        .presigned(presigning_config)
        .await
    {
        Ok(req) => req,
        Err(e) => {
            eprintln!("S3 presign error: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "S3 error").into_response();
        }
    };

    let full_uri = presigned_request.uri();
    let public_url = full_uri.replacen("/svh/", "/", 1);

    (StatusCode::OK, Json(json!({ "id": video_id.0, "upload_url": public_url }))).into_response()
}

