use anyhow::{Context, Result};
use aws_sdk_s3::config::Credentials;
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
}

#[derive(sqlx::Type, Debug, Clone, Copy, PartialEq, Eq)]
#[sqlx(type_name = "vid_obj_type", rename_all = "lowercase")]
enum VidObjType {
    Preview,
    Orig,
    High,
    Low,
}

impl VidObjType {
    fn as_filename(&self) -> &'static str {
        match self {
            VidObjType::Preview => "preview.jpg",
            VidObjType::Orig => "orig.mp4",
            VidObjType::High => "high.mp4",
            VidObjType::Low => "low.mp4",
        }
    }
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
    let s3_bucket = env::var("S3_PUBLIC_BUCKET").unwrap_or_else(|_| "svh".to_string());

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
    };

    tracing::info!("Transcoder started, polling every {}s", poll_interval);

    loop {
        match fetch_job(&state).await {
            Ok(Some((vid, job_type))) => {
                tracing::info!("Picked up job: vid={} type={:?}", vid, job_type);
                if let Err(e) = process_job(&state, vid, job_type).await {
                    tracing::error!("Job failed: vid={} type={:?} error={:?}", vid, job_type, e);
                } else {
                    if let Err(e) = complete_job(&state, vid, job_type).await {
                        tracing::error!("Failed to mark job complete: {:?}", e);
                    } else {
                        tracing::info!("Job done: vid={} type={:?}", vid, job_type);
                    }
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

async fn fetch_job(state: &AppState) -> Result<Option<(i16, VidObjType)>> {
    let row: Option<(i16, VidObjType)> =
        sqlx::query_as("SELECT job_vid, job_type FROM get_trn_job()")
            .fetch_optional(&state.db)
            .await?;

    Ok(row)
}

async fn complete_job(state: &AppState, vid: i16, job_type: VidObjType) -> Result<()> {
    sqlx::query("SELECT complete_trn_job($1, $2)")
        .bind(vid)
        .bind(job_type)
        .execute(&state.db)
        .await?;

    Ok(())
}

async fn process_job(state: &AppState, vid: i16, job_type: VidObjType) -> Result<()> {
    let work_dir = format!("/tmp/trn/{}", vid);
    tokio::fs::create_dir_all(&work_dir).await?;

    let result = process_job_inner(state, vid, job_type, &work_dir).await;

    let _ = tokio::fs::remove_dir_all(&work_dir).await;

    result
}

async fn process_job_inner(
    state: &AppState,
    vid: i16,
    job_type: VidObjType,
    work_dir: &str,
) -> Result<()> {
    let orig_key = format!("video/{}/orig.mp4", vid);
    let orig_path = format!("{}/orig.mp4", work_dir);

    download_object(state, &orig_key, &orig_path).await?;

    match job_type {
        VidObjType::Preview => {
            let out_path = format!("{}/preview.jpg", work_dir);
            make_preview(&orig_path, &out_path).await?;
            let key = format!("previews/{}.jpg", vid);
            upload_object(state, &out_path, &key, "image/jpeg").await?;
        }
        VidObjType::High => {
            let out_path = format!("{}/high.mp4", work_dir);
            make_variant(&orig_path, &out_path, 1080, 30, "8M").await?;
            let key = format!("video/{}/high.mp4", vid);
            upload_object(state, &out_path, &key, "video/mp4").await?;
        }
        VidObjType::Low => {
            let out_path = format!("{}/low.mp4", work_dir);
            make_variant(&orig_path, &out_path, 360, 30, "1M").await?;
            let key = format!("video/{}/low.mp4", vid);
            upload_object(state, &out_path, &key, "video/mp4").await?;
        }
        VidObjType::Orig => {
            anyhow::bail!("orig is not produced by the transcoder");
        }
    }

    Ok(())
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

async fn make_preview(input: &str, output: &str) -> Result<()> {
    run_ffmpeg(&[
        "-y", "-i", input,
        "-ss", "00:00:03",
        "-vframes", "1",
        output,
    ])
    .await
}

async fn make_variant(
    input: &str,
    output: &str,
    height: u32,
    fps: u32,
    bitrate: &str,
) -> Result<()> {
    run_ffmpeg(&[
        "-y", "-i", input,
        "-vf", &format!("scale=-2:{},fps={}", height, fps),
        "-c:v", "libx264",
        "-b:v", bitrate,
        "-c:a", "copy",
        output,
    ])
    .await
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

