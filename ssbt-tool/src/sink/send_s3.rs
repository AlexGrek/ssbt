use s3::bucket::Bucket;
use s3::creds::Credentials;
use s3::Region;
use tokio::io::DuplexStream;

/// Uploads data from a duplex reader to an S3-compatible bucket.
///
/// Uses streaming upload so the archive is never fully buffered in memory.
/// Credentials can be passed explicitly or resolved from standard AWS environment
/// variables (AWS_ACCESS_KEY_ID, AWS_SECRET_ACCESS_KEY).
pub async fn upload_to_s3(
    mut reader: DuplexStream,
    bucket_name: &str,
    key: &str,
    region: &str,
    endpoint: Option<&str>,
    access_key: Option<&str>,
    secret_key: Option<&str>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let region = match endpoint {
        Some(ep) => Region::Custom {
            region: region.to_string(),
            endpoint: ep.to_string(),
        },
        None => region.parse()?,
    };

    let credentials = match (access_key, secret_key) {
        (Some(ak), Some(sk)) => Credentials::new(Some(ak), Some(sk), None, None, None)?,
        _ => Credentials::from_env()?,
    };

    let bucket = Bucket::new(bucket_name, region, credentials)?;

    bucket.put_object_stream(&mut reader, key).await?;

    Ok(())
}
