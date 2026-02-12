use suppaftp::{AsyncNativeTlsConnector, AsyncNativeTlsFtpStream, AsyncFtpStream};
use tokio::io::DuplexStream;
use tokio_util::compat::TokioAsyncReadCompatExt;

/// Uploads data from a duplex reader to an FTP/FTPS server.
///
/// Uses the streaming `put_file` API so the archive is never fully buffered in memory.
pub async fn upload_via_ftp(
    reader: DuplexStream,
    host: &str,
    port: u16,
    remote_path: &str,
    user: &str,
    password: &str,
    secure: bool,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let addr = format!("{}:{}", host, port);
    let mut compat_reader = reader.compat();

    if secure {
        let connector = AsyncNativeTlsConnector::from(
            suppaftp::async_native_tls::TlsConnector::new(),
        );
        let ftp = AsyncNativeTlsFtpStream::connect(&addr).await?;
        let mut ftp = ftp.into_secure(connector, host).await?;
        ftp.login(user, password).await?;
        ftp.transfer_type(suppaftp::types::FileType::Binary).await?;
        ftp.put_file(remote_path, &mut compat_reader).await?;
        ftp.quit().await?;
    } else {
        let mut ftp = AsyncFtpStream::connect(&addr).await?;
        ftp.login(user, password).await?;
        ftp.transfer_type(suppaftp::types::FileType::Binary).await?;
        ftp.put_file(remote_path, &mut compat_reader).await?;
        ftp.quit().await?;
    }

    Ok(())
}
