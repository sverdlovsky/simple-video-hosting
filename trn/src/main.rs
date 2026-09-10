use anyhow::{Context, Result};
use aws_sdk_s3::config::Credentials;
use sqlx::postgres::PgPoolOptions;
use std::{
    env,
    process::Stdio,
    time::Duration
};
use tokio::process::Command;


#[derive(Clone)]
struct AppState {
    db: sqlx::Pool<sqlx::Postgres>,
    s3: aws_sdk_s3::Client,
    s3_bucket: String,
    lease_seconds: i32,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let dsn = env::var("DATABASE_URL").context("Environment variable DATABASE_URL is not set!")?;
    let pool = PgPoolOptions::new()
        .max_connections(num_cpus::get() as u32 * 2)
        .idle_timeout(Duration::from_secs(300))
        .connect(dsn.as_str())
        .await
        .context("Failed to connect to Postgres")?;
    let poll_interval = env::var("POLL_INTERVAL")
        .unwrap_or_else(|_| "5".to_string())
        .parse::<u64>()
        .context("POLL_INTERVAL must be a number")?;

    let s3_endpoint = env::var("S3_ENDPOINT").context("Environment variable S3_ENDPOINT is not set!")?;
    let s3_region = env::var("S3_REGION").unwrap_or_else(|_| "us-east-1".to_string());
    let s3_access_key = env::var("S3_ACCESS_KEY").context("Environment variable S3_ACCESS_KEY is not set!")?;
    let s3_secret_key = env::var("S3_SECRET_KEY").context("Environment variable S3_SECRET_KEY is not set!")?;
    let s3_bucket = env::var("S3_BUCKET").unwrap_or_else(|_| "svh".to_string());

    let lease_seconds = env::var("LEASE")
        .unwrap_or_else(|_| "900".to_string())
        .parse::<i32>()
        .context("LEASE_SECONDS must be a number")?;

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
        lease_seconds,
    };

    tracing::info!("Transcoder started, polling every {}s", poll_interval);

    loop {
        match fetch_job(&state).await {
            Ok(Some((kind, id))) => {
                tracing::info!("Picked up job: kind={} id={}", kind, id);
                if let Err(e) = process_job(&state, &kind, id).await {
                    tracing::error!("Job failed: kind={} id={} error={:?}", kind, id, e);
                } else if let Err(e) = complete_job(&state, &kind, id).await {
                    tracing::error!("Failed to delete task: {:?}", e);
                } else {
                    tracing::info!("Job done: kind={} id={}", kind, id);
                }
            }
            Ok(None) => {
                tokio::time::sleep(Duration::from_secs(poll_interval)).await;
            }
            Err(e) => {
                tracing::error!("Failed to fetch job: {:?}", e);
                tokio::time::sleep(Duration::from_secs(poll_interval)).await;
            }
        }
    }
}

async fn fetch_job(state: &AppState) -> Result<Option<(String, i16)>> {
    let row: Option<(String, i16)> = sqlx::query_as(
        "SELECT job_kind, job_id FROM get_trn_job($1)"
    )
        .bind(state.lease_seconds)
        .fetch_optional(&state.db)
        .await?;

    Ok(row)
}

async fn complete_job(state: &AppState, kind: &str, id: i16) -> Result<()> {
    sqlx::query("DELETE FROM Transcode_Tasks WHERE kind = $1 AND id = $2")
        .bind(kind)
        .bind(id)
        .execute(&state.db)
        .await?;

    Ok(())
}

async fn process_job(state: &AppState, kind: &str, id: i16) -> Result<()> {
    let work_dir = format!("/tmp/trn/{}_{}", kind, id);
    tokio::fs::create_dir_all(&work_dir).await?;

    let result = process_job_inner(state, kind, id, &work_dir).await;

    let _ = tokio::fs::remove_dir_all(&work_dir).await;

    result
}

async fn process_job_inner(state: &AppState, kind: &str, id: i16, work_dir: &str) -> Result<()> {
    match kind {
        "video" => process_video(state, id, work_dir).await,
        "preview" => process_preview(state, id, work_dir).await,
        "app" | "avatar" => process_square_image(state, kind, id, work_dir).await,
        other => anyhow::bail!("Unknown task kind: {}", other),
    }
}

async fn process_video(state: &AppState, id: i16, work_dir: &str) -> Result<()> {
    let orig_key = format!("video/{}/orig", id);
    let orig_path = format!("{}/orig.mp4", work_dir);
    download_object(state, &orig_key, &orig_path).await?;

    let high_path = format!("{}/high.mp4", work_dir);
    run_ffmpeg(&[
        "-y", "-i", &orig_path,
        "-vf", "scale=-2:1080,fps=60",
        "-c:v", "libx265",
        "-b:v", "8M",
        "-tag:v", "hvc1",
        "-c:a", "copy",
        &high_path,
    ])
    .await?;
    upload_object(state, &high_path, &format!("video/{}/high", id), "video/mp4").await?;

    let low_path = format!("{}/low.mp4", work_dir);
    run_ffmpeg(&[
        "-y", "-i", &orig_path,
        "-vf", "scale=-2:360,fps=30",
        "-c:v", "libx264",
        "-b:v", "1M",
        "-c:a", "copy",
        &low_path,
    ])
    .await?;
    upload_object(state, &low_path, &format!("video/{}/low", id), "video/mp4").await?;

    Ok(())
}

async fn process_preview(state: &AppState, id: i16, work_dir: &str) -> Result<()> {
    let preview_orig_key = format!("preview/{}/orig", id);
    let preview_orig_path = format!("{}/orig.png", work_dir);

    if !object_exists(state, &preview_orig_key).await? {
        let video_orig_key = format!("video/{}/orig", id);
        let video_orig_path = format!("{}/video_orig.mp4", work_dir);
        download_object(state, &video_orig_key, &video_orig_path).await?;

        run_ffmpeg(&[
            "-y",
            "-ss", "00:00:03",
            "-i", &video_orig_path,
            "-vframes", "1",
            "-quality", "90",
            &preview_orig_path,
        ])
        .await?;

        upload_object(state, &preview_orig_path, &preview_orig_key, "image/webp").await?;
    } else {
        download_object(state, &preview_orig_key, &preview_orig_path).await?;
    }

    let high_path = format!("{}/high.webp", work_dir);
    run_ffmpeg(&[
        "-y", "-i", &preview_orig_path,
        "-vf", "scale=1920:1080:force_original_aspect_ratio=decrease,pad=1920:1080:(ow-iw)/2:(oh-ih)/2:color=black@0",
        "-quality", "80",
        &high_path,
    ])
    .await?;
    upload_object(state, &high_path, &format!("preview/{}/high", id), "image/webp").await?;

    let low_path = format!("{}/low.webp", work_dir);
    run_ffmpeg(&[
        "-y", "-i", &preview_orig_path,
        "-vf", "scale=768:432:force_original_aspect_ratio=decrease,pad=768:432:(ow-iw)/2:(oh-ih)/2:color=black@0",
        "-quality", "80",
        &low_path,
    ])
    .await?;
    upload_object(state, &low_path, &format!("preview/{}/low", id), "image/webp").await?;

    Ok(())
}

async fn process_square_image(state: &AppState, kind: &str, id: i16, work_dir: &str) -> Result<()> {
    let orig_key = format!("{}/{}/orig", kind, id);
    let orig_path = format!("{}/orig", work_dir);
    download_object(state, &orig_key, &orig_path).await?;

    let high_path = format!("{}/high.webp", work_dir);
    run_ffmpeg(&[
        "-y", "-i", &orig_path,
        "-vf", "scale=512:512:force_original_aspect_ratio=decrease,pad=512:512:(ow-iw)/2:(oh-ih)/2:color=black@0",
        "-lossless", "1",
        &high_path,
    ])
    .await?;
    upload_object(state, &high_path, &format!("{}/{}/high", kind, id), "image/webp").await?;

    let low_path = format!("{}/low.webp", work_dir);
    run_ffmpeg(&[
        "-y", "-i", &orig_path,
        "-vf", "scale=64:64:force_original_aspect_ratio=decrease,pad=64:64:(ow-iw)/2:(oh-ih)/2:color=black@0",
        "-lossless", "1",
        &low_path,
    ])
    .await?;
    upload_object(state, &low_path, &format!("{}/{}/low", kind, id), "image/webp").await?;

    Ok(())
}

async fn object_exists(state: &AppState, key: &str) -> Result<bool> {
    match state.s3.head_object().bucket(&state.s3_bucket).key(key).send().await {
        Ok(_) => Ok(true),
        Err(e) => {
            if let Some(service_err) = e.as_service_error() {
                if service_err.is_not_found() {
                    return Ok(false);
                }
            }
            Err(e).context("Failed to check object existence")
        }
    }
}

async fn download_object(state: &AppState, key: &str, dest_path: &str) -> Result<()> {
    let mut obj = state
        .s3
        .get_object()
        .bucket(&state.s3_bucket)
        .key(key)
        .send()
        .await
        .context("Failed to start S3 download")?;

    let mut file = tokio::fs::File::create(dest_path).await?;

    while let Some(chunk) = obj.body.try_next().await? {
        tokio::io::AsyncWriteExt::write_all(&mut file, &chunk).await?;
    }

    Ok(())
}

async fn upload_object(state: &AppState, src_path: &str, key: &str, content_type: &str) -> Result<()> {
    let body = aws_sdk_s3::primitives::ByteStream::from_path(src_path)
        .await
        .context("Failed to read file for upload")?;

    state
        .s3
        .put_object()
        .bucket(&state.s3_bucket)
        .key(key)
        .content_type(content_type)
        .body(body)
        .send()
        .await
        .context("Failed to upload to S3")?;

    Ok(())
}

async fn run_ffmpeg(args: &[&str]) -> Result<()> {
    let output = Command::new("ffmpeg")
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .await
        .context("Failed to spawn ffmpeg")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("ffmpeg failed: {}", stderr);
    }

    Ok(())
}

